/**
 * Gafana - Ephemeral Stateless Signaling & Packet Protocol (Hardened)
 */

#include "gafana_internal.h"

gafana_error_t gafana_broadcast_packet(
    gafana_engine_t *engine,
    uint8_t opcode,
    const void *payload,
    uint16_t payload_len
) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    /* Transmit over real socket transport */
    return gafana_net_send(&engine->net,
                           engine->current_room_id,
                           engine->self_peer_id,
                           opcode,
                           payload,
                           payload_len);
}

gafana_error_t gafana_handle_packet(
    gafana_engine_t *engine,
    const uint8_t *buffer,
    size_t length
) {
    if (!engine || !buffer || length < sizeof(gafana_pkt_header_t)) {
        return GAFANA_ERR_INVALID_PARAM;
    }

    const gafana_pkt_header_t *hdr = (const gafana_pkt_header_t *)buffer;
    if (hdr->magic != GAFANA_MAGIC) {
        return GAFANA_ERR_INVALID_PARAM;
    }

    /* Ignore packets originating from ourselves */
    if (strncmp(hdr->sender_id, engine->self_peer_id, GAFANA_MAX_ID_LEN) == 0) {
        return GAFANA_OK;
    }

    const uint8_t *payload = buffer + sizeof(gafana_pkt_header_t);
    uint16_t plen = hdr->payload_len;

    /* Update peer heartbeat timer */
    gafana_mutex_lock(&engine->peer_lock);
    for (uint32_t i = 0; i < engine->peer_capacity; ++i) {
        if (engine->peers[i].active && strncmp(engine->peers[i].info.peer_id, hdr->sender_id, GAFANA_MAX_ID_LEN) == 0) {
            engine->peers[i].last_seen_ms = gafana_get_time_ms();
            break;
        }
    }
    gafana_mutex_unlock(&engine->peer_lock);

    switch (hdr->opcode) {
        case GAFANA_OP_HEARTBEAT:
            /* Heartbeat already updated last_seen_ms above */
            break;

        case GAFANA_OP_JOIN: {
            char name_buf[GAFANA_MAX_NAME_LEN] = "Participant";
            if (plen > 0) {
                strncpy(name_buf, (const char *)payload, GAFANA_MAX_NAME_LEN - 1);
            }

            gafana_mutex_lock(&engine->peer_lock);
            bool exists = false;
            int free_slot = -1;
            for (uint32_t i = 0; i < engine->peer_capacity; ++i) {
                if (engine->peers[i].active && strncmp(engine->peers[i].info.peer_id, hdr->sender_id, GAFANA_MAX_ID_LEN) == 0) {
                    exists = true;
                    engine->peers[i].last_seen_ms = gafana_get_time_ms();
                    break;
                }
                if (!engine->peers[i].active && free_slot == -1) {
                    free_slot = (int)i;
                }
            }

            if (!exists && free_slot != -1) {
                gafana_peer_node_t *node = &engine->peers[free_slot];
                memset(node, 0, sizeof(gafana_peer_node_t));
                strncpy(node->info.peer_id, hdr->sender_id, GAFANA_MAX_ID_LEN - 1);
                strncpy(node->info.display_name, name_buf, GAFANA_MAX_NAME_LEN - 1);
                node->info.join_timestamp_sec = (uint32_t)time(NULL);
                node->active = true;
                node->last_seen_ms = gafana_get_time_ms();
                engine->active_peer_count++;

                gafana_peer_info_t info_copy = node->info;
                gafana_mutex_unlock(&engine->peer_lock);

                /* If local recording is active when a new peer joins, privacy safety requires either
                 * consent ballot or pause. */
                gafana_mutex_lock(&engine->recorder.lock);
                if (engine->recorder.is_recording) {
                    printf("[PRIVACY ENFORCER] New peer '%s' joined during active recording. Halting capture until consensus reaffirmed.\n", hdr->sender_id);
                    gafana_mutex_unlock(&engine->recorder.lock);
                    gafana_recording_stop_local(engine);
                } else {
                    gafana_mutex_unlock(&engine->recorder.lock);
                }

                gafana_mutex_lock(&engine->ballot_lock);
                if (engine->current_ballot.is_active && engine->current_ballot.state == GAFANA_CONSENT_REQUESTED) {
                    engine->current_ballot.total_expected_votes++;
                }
                gafana_mutex_unlock(&engine->ballot_lock);

                if (engine->cbs.on_peer_joined) {
                    engine->cbs.on_peer_joined(engine, &info_copy, engine->user_data);
                }
            } else {
                gafana_mutex_unlock(&engine->peer_lock);
            }
            break;
        }

        case GAFANA_OP_LEAVE: {
            gafana_mutex_lock(&engine->peer_lock);
            for (uint32_t i = 0; i < engine->peer_capacity; ++i) {
                if (engine->peers[i].active && strncmp(engine->peers[i].info.peer_id, hdr->sender_id, GAFANA_MAX_ID_LEN) == 0) {
                    engine->peers[i].active = false;
                    if (engine->active_peer_count > 0) engine->active_peer_count--;
                    break;
                }
            }
            gafana_mutex_unlock(&engine->peer_lock);

            if (engine->cbs.on_peer_left) {
                engine->cbs.on_peer_left(engine, hdr->sender_id, "UserDisconnected", engine->user_data);
            }
            break;
        }

        case GAFANA_OP_TRACK_ANNOUNCE: {
            if (plen < sizeof(uint32_t) * 2) break;
            const uint32_t *tdata = (const uint32_t *)payload;
            gafana_track_type_t ttype = (gafana_track_type_t)tdata[0];
            uint32_t tid = tdata[1];

            if (engine->cbs.on_track_published) {
                engine->cbs.on_track_published(engine, hdr->sender_id, ttype, tid, engine->user_data);
            }
            break;
        }

        case GAFANA_OP_TRACK_REMOVE: {
            if (plen < sizeof(uint32_t)) break;
            uint32_t tid = *(const uint32_t *)payload;

            if (engine->cbs.on_track_unpublished) {
                engine->cbs.on_track_unpublished(engine, hdr->sender_id, GAFANA_TRACK_AUDIO, tid, engine->user_data);
            }
            break;
        }

        case GAFANA_OP_CONSENT_REQUEST: {
            if (plen < sizeof(gafana_pkt_consent_req_t)) break;
            const gafana_pkt_consent_req_t *req = (const gafana_pkt_consent_req_t *)payload;

            gafana_mutex_lock(&engine->ballot_lock);
            engine->current_ballot.ballot_id = req->ballot_id;
            strncpy(engine->current_ballot.requester_id, req->requester_id, GAFANA_MAX_ID_LEN - 1);
            engine->current_ballot.rec_type = (gafana_recording_type_t)req->rec_type;
            engine->current_ballot.state = GAFANA_CONSENT_REQUESTED;
            uint32_t now = gafana_get_time_ms();
            engine->current_ballot.start_time_ms = now;
            engine->current_ballot.expires_at_ms = now + (req->timeout_sec * 1000);
            engine->current_ballot.is_active = true;
            gafana_mutex_unlock(&engine->ballot_lock);

            if (engine->cbs.on_recording_consent_requested) {
                engine->cbs.on_recording_consent_requested(
                    engine,
                    req->ballot_id,
                    req->requester_id,
                    (gafana_recording_type_t)req->rec_type,
                    engine->user_data
                );
            }
            break;
        }

        case GAFANA_OP_CONSENT_VOTE: {
            if (plen < sizeof(gafana_pkt_consent_vote_t)) break;
            const gafana_pkt_consent_vote_t *v = (const gafana_pkt_consent_vote_t *)payload;

            gafana_mutex_lock(&engine->ballot_lock);
            if (engine->current_ballot.is_active && engine->current_ballot.ballot_id == v->ballot_id) {
                if (v->vote == 0) {
                    /* VETO: Single participant NO rejects recording */
                    engine->current_ballot.state = GAFANA_CONSENT_REJECTED;
                    uint32_t bid = engine->current_ballot.ballot_id;
                    char req_id[GAFANA_MAX_ID_LEN];
                    strncpy(req_id, engine->current_ballot.requester_id, GAFANA_MAX_ID_LEN - 1);
                    gafana_mutex_unlock(&engine->ballot_lock);

                    if (engine->cbs.on_recording_state_changed) {
                        engine->cbs.on_recording_state_changed(engine, bid, GAFANA_CONSENT_REJECTED, req_id, engine->user_data);
                    }
                } else {
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
                }
            } else {
                gafana_mutex_unlock(&engine->ballot_lock);
            }
            break;
        }

        case GAFANA_OP_CONSENT_REVOKE: {
            if (plen < sizeof(gafana_pkt_consent_revoke_t)) break;
            const gafana_pkt_consent_revoke_t *rev = (const gafana_pkt_consent_revoke_t *)payload;

            /* Force immediate local recorder shutdown */
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
                engine->cbs.on_recording_state_changed(engine, rev->ballot_id, GAFANA_CONSENT_REVOKED, rev->revoker_id, engine->user_data);
            }
            break;
        }

        case GAFANA_OP_MEDIA_VIDEO: {
            if (plen == 0) break;
            gafana_video_frame_t frame;
            frame.data = payload;
            frame.size = plen;
            frame.width = 1280;
            frame.height = 720;
            frame.stride = 1280 * 4;
            frame.timestamp_ms = hdr->timestamp;
            frame.fourcc = 0;
            frame.is_keyframe = false;

            if (engine->cbs.on_video_frame) {
                engine->cbs.on_video_frame(engine, hdr->sender_id, GAFANA_TRACK_VIDEO, &frame, engine->user_data);
            }
            break;
        }

        case GAFANA_OP_MEDIA_AUDIO: {
            if (plen < sizeof(int16_t)) break;
            gafana_audio_frame_t aframe;
            aframe.pcm_samples = (const int16_t *)payload;
            aframe.channels = 2;
            aframe.sample_count = plen / (sizeof(int16_t) * 2);
            aframe.sample_rate = 48000;
            aframe.timestamp_ms = hdr->timestamp;

            if (engine->cbs.on_audio_frame) {
                engine->cbs.on_audio_frame(engine, hdr->sender_id, &aframe, engine->user_data);
            }
            break;
        }

        case GAFANA_OP_PRES_EVENT: {
            if (plen < sizeof(gafana_pkt_pres_t)) break;
            const gafana_pkt_pres_t *p = (const gafana_pkt_pres_t *)payload;
            gafana_presentation_event_t ev;
            ev.type = (gafana_pres_event_type_t)p->event_type;
            ev.current_slide = p->current_slide;
            ev.total_slides = p->total_slides;
            ev.pointer_x = p->pointer_x;
            ev.pointer_y = p->pointer_y;
            ev.color_hex = p->color_hex;

            if (engine->cbs.on_presentation_event) {
                engine->cbs.on_presentation_event(engine, hdr->sender_id, &ev, engine->user_data);
            }
            break;
        }

        default:
            break;
    }

    return GAFANA_OK;
}
