/**
 * Gafana - Core Engine Implementation (Hardened Production Version)
 */

#include "gafana_internal.h"

static const char *s_version = GAFANA_VERSION_STRING;

const char *gafana_get_version_string(void) {
    return s_version;
}

void gafana_config_default(gafana_config_t *config) {
    if (!config) return;
    memset(config, 0, sizeof(gafana_config_t));
    snprintf(config->relay_host, sizeof(config->relay_host), "127.0.0.1");
    config->relay_port = GAFANA_DEFAULT_PORT;
    config->relay_mode = GAFANA_RELAY_STATELESS_SFU;
    config->max_peers = GAFANA_MAX_ROOM_PEERS;
    config->video_bitrate_kbps = 2500;
    config->audio_bitrate_kbps = 64;
    config->ballot_timeout_sec = GAFANA_BALLOT_TIMEOUT_SEC;
    config->enable_e2ee = true;
    config->e2ee_passphrase[0] = '\0';
}

uint32_t gafana_get_time_ms(void) {
#if defined(_WIN32)
    return (uint32_t)GetTickCount();
#else
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint32_t)((ts.tv_sec * 1000) + (ts.tv_nsec / 1000000));
#endif
}

gafana_error_t gafana_engine_create(
    const gafana_config_t *config,
    const gafana_callbacks_t *callbacks,
    void *user_data,
    gafana_engine_t **out_engine
) {
    if (!out_engine) return GAFANA_ERR_INVALID_PARAM;

    gafana_engine_t *engine = (gafana_engine_t *)calloc(1, sizeof(gafana_engine_t));
    if (!engine) return GAFANA_ERR_OUT_OF_MEMORY;

    if (config) {
        memcpy(&engine->config, config, sizeof(gafana_config_t));
    } else {
        gafana_config_default(&engine->config);
    }

    if (callbacks) {
        memcpy(&engine->cbs, callbacks, sizeof(gafana_callbacks_t));
    }
    engine->user_data = user_data;

    uint32_t cap = engine->config.max_peers > 0 ? engine->config.max_peers : GAFANA_MAX_ROOM_PEERS;
    engine->peer_capacity = cap;
    engine->peers = (gafana_peer_node_t *)calloc(cap, sizeof(gafana_peer_node_t));
    if (!engine->peers) {
        free(engine);
        return GAFANA_ERR_OUT_OF_MEMORY;
    }

    gafana_mutex_init(&engine->peer_lock);
    gafana_mutex_init(&engine->track_lock);
    gafana_mutex_init(&engine->ballot_lock);
    gafana_mutex_init(&engine->recorder.lock);
    gafana_mutex_init(&engine->pres_lock);

    engine->next_track_id = 1;
    engine->next_ballot_id = 100;
    engine->is_running = true;

    *out_engine = engine;
    return GAFANA_OK;
}

void gafana_engine_destroy(gafana_engine_t *engine) {
    if (!engine) return;

    if (engine->in_room) {
        gafana_room_leave(engine);
    }

    if (engine->recorder.is_recording) {
        gafana_recording_stop_local(engine);
    }

    gafana_net_close(&engine->net);

    engine->is_running = false;

    gafana_mutex_free(&engine->peer_lock);
    gafana_mutex_free(&engine->track_lock);
    gafana_mutex_free(&engine->ballot_lock);
    gafana_mutex_free(&engine->recorder.lock);
    gafana_mutex_free(&engine->pres_lock);

    if (engine->peers) {
        /* Secure zeroize peer table before free */
        memset(engine->peers, 0, engine->peer_capacity * sizeof(gafana_peer_node_t));
        free(engine->peers);
        engine->peers = NULL;
    }

    memset(engine, 0, sizeof(gafana_engine_t));
    free(engine);
}

gafana_error_t gafana_room_join(
    gafana_engine_t *engine,
    const char *room_id,
    const char *self_peer_id,
    const char *display_name
) {
    if (!engine || !room_id || !self_peer_id) return GAFANA_ERR_INVALID_PARAM;
    if (engine->in_room) return GAFANA_ERR_ALREADY_IN_ROOM;

    /* Initialize socket transport to relay */
    gafana_error_t net_err = gafana_net_init(&engine->net, engine->config.relay_host, engine->config.relay_port);
    if (net_err != GAFANA_OK && engine->config.relay_mode == GAFANA_RELAY_STATELESS_SFU) {
        if (engine->cbs.on_error) {
            engine->cbs.on_error(engine, net_err, "Failed to initialize network transport socket", engine->user_data);
        }
    }

    strncpy(engine->current_room_id, room_id, GAFANA_MAX_ID_LEN - 1);
    strncpy(engine->self_peer_id, self_peer_id, GAFANA_MAX_ID_LEN - 1);
    if (display_name && strlen(display_name) > 0) {
        strncpy(engine->self_display_name, display_name, GAFANA_MAX_NAME_LEN - 1);
    } else {
        snprintf(engine->self_display_name, sizeof(engine->self_display_name), "User_%s", self_peer_id);
    }

    engine->in_room = true;
    engine->last_heartbeat_sent_ms = gafana_get_time_ms();

    /* Self registered in peer list */
    gafana_mutex_lock(&engine->peer_lock);
    strncpy(engine->peers[0].info.peer_id, self_peer_id, GAFANA_MAX_ID_LEN - 1);
    strncpy(engine->peers[0].info.display_name, engine->self_display_name, GAFANA_MAX_NAME_LEN - 1);
    engine->peers[0].info.join_timestamp_sec = (uint32_t)time(NULL);
    engine->peers[0].active = true;
    engine->peers[0].last_seen_ms = gafana_get_time_ms();
    engine->active_peer_count = 1;
    gafana_mutex_unlock(&engine->peer_lock);

    /* Broadcast JOIN packet over wire */
    gafana_broadcast_packet(engine, GAFANA_OP_JOIN, engine->self_display_name, (uint16_t)strlen(engine->self_display_name) + 1);

    if (engine->cbs.on_room_joined) {
        engine->cbs.on_room_joined(engine, room_id, self_peer_id, engine->user_data);
    }
    if (engine->cbs.on_connection_state) {
        engine->cbs.on_connection_state(engine, true, "Joined ephemeral room", engine->user_data);
    }

    return GAFANA_OK;
}

gafana_error_t gafana_room_leave(gafana_engine_t *engine) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    if (engine->recorder.is_recording) {
        gafana_recording_stop_local(engine);
    }

    gafana_broadcast_packet(engine, GAFANA_OP_LEAVE, "UserDeparted", 12);
    gafana_net_close(&engine->net);

    gafana_mutex_lock(&engine->peer_lock);
    for (uint32_t i = 0; i < engine->peer_capacity; ++i) {
        engine->peers[i].active = false;
    }
    engine->active_peer_count = 0;
    gafana_mutex_unlock(&engine->peer_lock);

    gafana_mutex_lock(&engine->track_lock);
    for (int i = 0; i < 64; ++i) {
        engine->tracks[i].is_active = false;
    }
    gafana_mutex_unlock(&engine->track_lock);

    engine->in_room = false;
    engine->current_room_id[0] = '\0';

    if (engine->cbs.on_connection_state) {
        engine->cbs.on_connection_state(engine, false, "Disconnected from room", engine->user_data);
    }

    return GAFANA_OK;
}

gafana_error_t gafana_engine_poll(gafana_engine_t *engine) {
    if (!engine || !engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    uint32_t now = gafana_get_time_ms();

    /* 1. Receive incoming datagrams from socket (non-blocking) */
    uint8_t buffer[GAFANA_MAX_PACKET_SIZE];
    gafana_net_endpoint_t src;
    int received = 0;

    while ((received = gafana_net_recv(&engine->net, buffer, sizeof(buffer), &src)) > 0) {
        /* Skip the network header and dispatch packet to engine */
        if ((size_t)received >= sizeof(gafana_net_packet_header_t)) {
            gafana_net_packet_header_t *nhdr = (gafana_net_packet_header_t *)buffer;
            /* Construct internal packet format */
            gafana_pkt_header_t ihdr;
            ihdr.magic = nhdr->magic;
            ihdr.opcode = nhdr->opcode;
            ihdr.flags = (uint8_t)nhdr->flags;
            ihdr.payload_len = nhdr->payload_len;
            ihdr.seq_num = nhdr->seq;
            ihdr.timestamp = nhdr->timestamp_ms;
            uint8_t ibuf[sizeof(gafana_pkt_header_t) + GAFANA_MAX_PACKET_SIZE];
            if (sizeof(ihdr) + nhdr->payload_len <= sizeof(ibuf)) {
                memcpy(ibuf, &ihdr, sizeof(ihdr));
                memcpy(ibuf + sizeof(ihdr), buffer + sizeof(gafana_net_packet_header_t), nhdr->payload_len);
                gafana_handle_packet(engine, ibuf, sizeof(ihdr) + nhdr->payload_len);
            }
        }
    }

    /* 2. Send heartbeat every 3 seconds */
    if (now - engine->last_heartbeat_sent_ms >= 3000) {
        gafana_broadcast_packet(engine, GAFANA_OP_HEARTBEAT, NULL, 0);
        engine->last_heartbeat_sent_ms = now;
    }

    /* 3. Check for ballot timeouts */
    gafana_consensus_check_timeouts(engine);

    /* 4. Detect dead peers (timeout 15 seconds) */
    gafana_mutex_lock(&engine->peer_lock);
    for (uint32_t i = 1; i < engine->peer_capacity; ++i) { /* index 0 is self */
        if (engine->peers[i].active && (now - engine->peers[i].last_seen_ms > 15000)) {
            char pid_copy[GAFANA_MAX_ID_LEN];
            strncpy(pid_copy, engine->peers[i].info.peer_id, GAFANA_MAX_ID_LEN - 1);
            engine->peers[i].active = false;
            if (engine->active_peer_count > 1) engine->active_peer_count--;

            gafana_mutex_unlock(&engine->peer_lock);

            if (engine->cbs.on_peer_left) {
                engine->cbs.on_peer_left(engine, pid_copy, "HeartbeatTimeout", engine->user_data);
            }

            gafana_mutex_lock(&engine->peer_lock);
        }
    }
    gafana_mutex_unlock(&engine->peer_lock);

    return GAFANA_OK;
}

uint32_t gafana_get_peer_count(gafana_engine_t *engine) {
    if (!engine || !engine->in_room) return 0;
    gafana_mutex_lock(&engine->peer_lock);
    uint32_t count = engine->active_peer_count;
    gafana_mutex_unlock(&engine->peer_lock);
    return count;
}

gafana_error_t gafana_get_peers(
    gafana_engine_t *engine,
    gafana_peer_info_t *out_peers,
    uint32_t max_count,
    uint32_t *out_count
) {
    if (!engine || !out_peers || !out_count) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->peer_lock);
    uint32_t copied = 0;
    for (uint32_t i = 0; i < engine->peer_capacity && copied < max_count; ++i) {
        if (engine->peers[i].active) {
            memcpy(&out_peers[copied], &engine->peers[i].info, sizeof(gafana_peer_info_t));
            copied++;
        }
    }
    *out_count = copied;
    gafana_mutex_unlock(&engine->peer_lock);

    return GAFANA_OK;
}
