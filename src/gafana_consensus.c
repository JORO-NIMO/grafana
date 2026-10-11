/**
 * Gafana - Unanimous Consensus & Consent-Driven Local Recording Subsystem (Hardened)
 */

#include "gafana_internal.h"

#pragma pack(push, 1)
typedef struct wav_header_s {
    char     riff_tag[4];       /* "RIFF" */
    uint32_t riff_size;
    char     wave_tag[4];       /* "WAVE" */
    char     fmt_tag[4];        /* "fmt " */
    uint32_t fmt_size;          /* 16 for PCM */
    uint16_t audio_format;      /* 1 for PCM */
    uint16_t num_channels;      /* 1 mono, 2 stereo */
    uint32_t sample_rate;       /* e.g. 48000 */
    uint32_t byte_rate;         /* sample_rate * num_channels * bits_per_sample / 8 */
    uint16_t block_align;       /* num_channels * bits_per_sample / 8 */
    uint16_t bits_per_sample;   /* 16 */
    char     data_tag[4];       /* "data" */
    uint32_t data_size;
} wav_header_t;
#pragma pack(pop)

void gafana_consensus_reset(gafana_engine_t *engine) {
    if (!engine) return;
    gafana_mutex_lock(&engine->ballot_lock);
    memset(&engine->current_ballot, 0, sizeof(gafana_ballot_t));
    engine->current_ballot.state = GAFANA_CONSENT_IDLE;
    gafana_mutex_unlock(&engine->ballot_lock);

    gafana_mutex_lock(&engine->peer_lock);
    for (uint32_t i = 0; i < engine->peer_capacity; ++i) {
        engine->peers[i].has_voted = false;
        engine->peers[i].info.has_granted_recording = false;
    }
    gafana_mutex_unlock(&engine->peer_lock);
}

void gafana_consensus_check_timeouts(gafana_engine_t *engine) {
    if (!engine) return;

    gafana_mutex_lock(&engine->ballot_lock);
    if (engine->current_ballot.is_active && engine->current_ballot.state == GAFANA_CONSENT_REQUESTED) {
        uint32_t now = gafana_get_time_ms();
        if (now >= engine->current_ballot.expires_at_ms) {
            uint32_t bid = engine->current_ballot.ballot_id;
            char req_id[GAFANA_MAX_ID_LEN];
            strncpy(req_id, engine->current_ballot.requester_id, GAFANA_MAX_ID_LEN - 1);
            engine->current_ballot.state = GAFANA_CONSENT_EXPIRED;
            engine->current_ballot.is_active = false;
            gafana_mutex_unlock(&engine->ballot_lock);

            printf("[CONSENSUS] Ballot #%u expired due to timeout. Consent denied by default.\n", bid);

            if (engine->cbs.on_recording_state_changed) {
                engine->cbs.on_recording_state_changed(engine, bid, GAFANA_CONSENT_EXPIRED, req_id, engine->user_data);
            }
            return;
        }
    }
    gafana_mutex_unlock(&engine->ballot_lock);
}

gafana_error_t gafana_recording_request_consent(
    gafana_engine_t *engine,
    gafana_recording_type_t rec_type,
    uint32_t *out_ballot_id
) {
    if (!engine || !out_ballot_id) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->recorder.lock);
    if (engine->recorder.is_recording) {
        gafana_mutex_unlock(&engine->recorder.lock);
        return GAFANA_ERR_RECORDING_ACTIVE;
    }
    gafana_mutex_unlock(&engine->recorder.lock);

    gafana_mutex_lock(&engine->ballot_lock);
    if (engine->current_ballot.is_active && engine->current_ballot.state == GAFANA_CONSENT_REQUESTED) {
        gafana_mutex_unlock(&engine->ballot_lock);
        return GAFANA_ERR_CONSENT_PENDING;
    }

    uint32_t ballot_id = engine->next_ballot_id++;
    engine->current_ballot.ballot_id = ballot_id;
    strncpy(engine->current_ballot.requester_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);
    engine->current_ballot.rec_type = rec_type;
    uint32_t now = gafana_get_time_ms();
    engine->current_ballot.start_time_ms = now;
    uint32_t timeout_sec = engine->config.ballot_timeout_sec > 0 ? engine->config.ballot_timeout_sec : GAFANA_BALLOT_TIMEOUT_SEC;
    engine->current_ballot.expires_at_ms = now + (timeout_sec * 1000);
    engine->current_ballot.is_active = true;

    /* Count active participants */
    gafana_mutex_lock(&engine->peer_lock);
    uint32_t total = engine->active_peer_count;
    /* Self automatically approves own request */
    engine->peers[0].has_voted = true;
    engine->peers[0].vote_status = GAFANA_VOTE_ACCEPT;
    engine->peers[0].info.has_granted_recording = true;
    gafana_mutex_unlock(&engine->peer_lock);

    engine->current_ballot.total_expected_votes = total;
    engine->current_ballot.yes_votes = 1;
    engine->current_ballot.no_votes = 0;

    if (total <= 1) {
        /* Solo participant: unanimous consent */
        engine->current_ballot.state = GAFANA_CONSENT_UNANIMOUS_OK;
        gafana_mutex_unlock(&engine->ballot_lock);

        if (engine->cbs.on_recording_vote_update) {
            engine->cbs.on_recording_vote_update(engine, ballot_id, 1, 1, engine->user_data);
        }
        if (engine->cbs.on_recording_state_changed) {
            engine->cbs.on_recording_state_changed(engine, ballot_id, GAFANA_CONSENT_UNANIMOUS_OK, engine->self_peer_id, engine->user_data);
        }
    } else {
        engine->current_ballot.state = GAFANA_CONSENT_REQUESTED;
        gafana_mutex_unlock(&engine->ballot_lock);

        gafana_pkt_consent_req_t req_pkt;
        req_pkt.ballot_id = ballot_id;
        req_pkt.rec_type = (uint8_t)rec_type;
        req_pkt.timeout_sec = timeout_sec;
        strncpy(req_pkt.requester_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);

        gafana_broadcast_packet(engine, GAFANA_OP_CONSENT_REQUEST, &req_pkt, sizeof(req_pkt));

        if (engine->cbs.on_recording_vote_update) {
            engine->cbs.on_recording_vote_update(engine, ballot_id, 1, total, engine->user_data);
        }
        if (engine->cbs.on_recording_state_changed) {
            engine->cbs.on_recording_state_changed(engine, ballot_id, GAFANA_CONSENT_REQUESTED, engine->self_peer_id, engine->user_data);
        }
    }

    *out_ballot_id = ballot_id;
    return GAFANA_OK;
}

gafana_error_t gafana_recording_cast_vote(
    gafana_engine_t *engine,
    uint32_t ballot_id,
    gafana_vote_t vote
) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->ballot_lock);
    if (!engine->current_ballot.is_active || engine->current_ballot.ballot_id != ballot_id) {
        gafana_mutex_unlock(&engine->ballot_lock);
        return GAFANA_ERR_INVALID_PARAM;
    }

    if (engine->current_ballot.state != GAFANA_CONSENT_REQUESTED) {
        gafana_mutex_unlock(&engine->ballot_lock);
        return GAFANA_ERR_UNAUTHORIZED;
    }

    gafana_pkt_consent_vote_t vote_pkt;
    vote_pkt.ballot_id = ballot_id;
    vote_pkt.vote = (uint8_t)vote;
    strncpy(vote_pkt.voter_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);
    gafana_broadcast_packet(engine, GAFANA_OP_CONSENT_VOTE, &vote_pkt, sizeof(vote_pkt));

    if (vote == GAFANA_VOTE_DENY) {
        engine->current_ballot.no_votes++;
        engine->current_ballot.state = GAFANA_CONSENT_REJECTED;
        uint32_t bid = engine->current_ballot.ballot_id;
        char req_id[GAFANA_MAX_ID_LEN];
        strncpy(req_id, engine->current_ballot.requester_id, GAFANA_MAX_ID_LEN - 1);
        gafana_mutex_unlock(&engine->ballot_lock);

        if (engine->cbs.on_recording_state_changed) {
            engine->cbs.on_recording_state_changed(engine, bid, GAFANA_CONSENT_REJECTED, req_id, engine->user_data);
        }
        return GAFANA_OK;
    }

    engine->current_ballot.yes_votes++;
    uint32_t yes = engine->current_ballot.yes_votes;
    uint32_t total = engine->current_ballot.total_expected_votes;
    bool unanimous = (yes >= total);

    if (unanimous) {
        engine->current_ballot.state = GAFANA_CONSENT_UNANIMOUS_OK;
    }
    uint32_t bid = engine->current_ballot.ballot_id;
    char req_id[GAFANA_MAX_ID_LEN];
    strncpy(req_id, engine->current_ballot.requester_id, GAFANA_MAX_ID_LEN - 1);
    gafana_mutex_unlock(&engine->ballot_lock);

    if (engine->cbs.on_recording_vote_update) {
        engine->cbs.on_recording_vote_update(engine, bid, yes, total, engine->user_data);
    }

    if (unanimous && engine->cbs.on_recording_state_changed) {
        engine->cbs.on_recording_state_changed(engine, bid, GAFANA_CONSENT_UNANIMOUS_OK, req_id, engine->user_data);
    }

    return GAFANA_OK;
}

gafana_consent_state_t gafana_recording_get_consent_state(
    gafana_engine_t *engine,
    uint32_t ballot_id
) {
    if (!engine) return GAFANA_CONSENT_IDLE;
    gafana_mutex_lock(&engine->ballot_lock);
    gafana_consent_state_t st = GAFANA_CONSENT_IDLE;
    if (engine->current_ballot.ballot_id == ballot_id) {
        st = engine->current_ballot.state;
    }
    gafana_mutex_unlock(&engine->ballot_lock);
    return st;
}

gafana_error_t gafana_recording_start_local(
    gafana_engine_t *engine,
    uint32_t ballot_id,
    const char *output_file_path
) {
    if (!engine || !output_file_path) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->ballot_lock);
    if (engine->current_ballot.ballot_id != ballot_id) {
        gafana_mutex_unlock(&engine->ballot_lock);
        return GAFANA_ERR_INVALID_PARAM;
    }

    if (engine->current_ballot.state != GAFANA_CONSENT_UNANIMOUS_OK) {
        gafana_consent_state_t st = engine->current_ballot.state;
        gafana_mutex_unlock(&engine->ballot_lock);
        return (st == GAFANA_CONSENT_REQUESTED) ? GAFANA_ERR_CONSENT_PENDING : GAFANA_ERR_CONSENT_DENIED;
    }
    gafana_recording_type_t rec_type = engine->current_ballot.rec_type;
    gafana_mutex_unlock(&engine->ballot_lock);

    gafana_mutex_lock(&engine->recorder.lock);
    if (engine->recorder.is_recording) {
        gafana_mutex_unlock(&engine->recorder.lock);
        return GAFANA_ERR_RECORDING_ACTIVE;
    }

    FILE *fp = fopen(output_file_path, "wb");
    if (!fp) {
        gafana_mutex_unlock(&engine->recorder.lock);
        return GAFANA_ERR_IO;
    }

    engine->recorder.fp = fp;
    engine->recorder.ballot_id = ballot_id;
    engine->recorder.rec_type = rec_type;
    strncpy(engine->recorder.file_path, output_file_path, GAFANA_MAX_PATH_LEN - 1);
    engine->recorder.bytes_written = 0;
    engine->recorder.frames_written = 0;
    engine->recorder.start_timestamp_ms = gafana_get_time_ms();
    engine->recorder.is_recording = true;

    if (rec_type == GAFANA_REC_AUDIO_ONLY) {
        wav_header_t hdr;
        memcpy(hdr.riff_tag, "RIFF", 4);
        hdr.riff_size = 36;
        memcpy(hdr.wave_tag, "WAVE", 4);
        memcpy(hdr.fmt_tag, "fmt ", 4);
        hdr.fmt_size = 16;
        hdr.audio_format = 1;
        hdr.num_channels = 2;
        hdr.sample_rate = 48000;
        hdr.bits_per_sample = 16;
        hdr.byte_rate = 48000 * 2 * 2;
        hdr.block_align = 2 * 2;
        memcpy(hdr.data_tag, "data", 4);
        hdr.data_size = 0;

        fwrite(&hdr, 1, sizeof(wav_header_t), fp);
        fflush(fp);
        engine->recorder.bytes_written += sizeof(wav_header_t);
    } else {
        const char *magic_banner = "GAFANA_AV_CONTAINER_V1\n";
        fwrite(magic_banner, 1, strlen(magic_banner), fp);
        fflush(fp);
        engine->recorder.bytes_written += strlen(magic_banner);
    }

    gafana_mutex_unlock(&engine->recorder.lock);
    return GAFANA_OK;
}

gafana_error_t gafana_recording_stop_local(gafana_engine_t *engine) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;

    gafana_mutex_lock(&engine->recorder.lock);
    if (!engine->recorder.is_recording || !engine->recorder.fp) {
        gafana_mutex_unlock(&engine->recorder.lock);
        return GAFANA_ERR_RECORDING_INACTIVE;
    }

    FILE *fp = engine->recorder.fp;

    if (engine->recorder.rec_type == GAFANA_REC_AUDIO_ONLY && engine->recorder.bytes_written >= sizeof(wav_header_t)) {
        uint32_t data_bytes = (uint32_t)(engine->recorder.bytes_written - sizeof(wav_header_t));
        uint32_t riff_bytes = data_bytes + 36;

        fseek(fp, 4, SEEK_SET);
        fwrite(&riff_bytes, 1, sizeof(uint32_t), fp);

        fseek(fp, 40, SEEK_SET);
        fwrite(&data_bytes, 1, sizeof(uint32_t), fp);
    }

    fflush(fp);
    fclose(fp);
    engine->recorder.fp = NULL;
    engine->recorder.is_recording = false;
    uint32_t ballot_id = engine->recorder.ballot_id;
    gafana_mutex_unlock(&engine->recorder.lock);

    gafana_consensus_reset(engine);

    if (engine->cbs.on_recording_state_changed) {
        engine->cbs.on_recording_state_changed(engine, ballot_id, GAFANA_CONSENT_IDLE, engine->self_peer_id, engine->user_data);
    }

    return GAFANA_OK;
}

gafana_error_t gafana_recording_revoke_consent(gafana_engine_t *engine, uint32_t ballot_id) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_pkt_consent_revoke_t rev_pkt;
    rev_pkt.ballot_id = ballot_id;
    strncpy(rev_pkt.revoker_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);
    gafana_broadcast_packet(engine, GAFANA_OP_CONSENT_REVOKE, &rev_pkt, sizeof(rev_pkt));

    gafana_mutex_lock(&engine->recorder.lock);
    if (engine->recorder.is_recording) {
        gafana_mutex_unlock(&engine->recorder.lock);
        gafana_recording_stop_local(engine);
    } else {
        gafana_mutex_unlock(&engine->recorder.lock);
    }

    gafana_mutex_lock(&engine->ballot_lock);
    engine->current_ballot.state = GAFANA_CONSENT_REVOKED;
    gafana_mutex_unlock(&engine->ballot_lock);

    if (engine->cbs.on_recording_state_changed) {
        engine->cbs.on_recording_state_changed(engine, ballot_id, GAFANA_CONSENT_REVOKED, engine->self_peer_id, engine->user_data);
    }

    return GAFANA_OK;
}

bool gafana_recording_is_active(gafana_engine_t *engine) {
    if (!engine) return false;
    gafana_mutex_lock(&engine->recorder.lock);
    bool active = engine->recorder.is_recording;
    gafana_mutex_unlock(&engine->recorder.lock);
    return active;
}
