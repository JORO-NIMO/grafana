/**
 * Gafana - Media Track Management, Screen Sharing, & Presentation Engine
 */

#include "gafana_internal.h"

gafana_error_t gafana_track_publish(
    gafana_engine_t *engine,
    gafana_track_type_t track_type,
    uint32_t *out_track_id
) {
    if (!engine || !out_track_id) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->track_lock);
    int slot = -1;
    for (int i = 0; i < 64; ++i) {
        if (!engine->tracks[i].is_active) {
            slot = i;
            break;
        }
    }

    if (slot == -1) {
        gafana_mutex_unlock(&engine->track_lock);
        return GAFANA_ERR_OUT_OF_MEMORY;
    }

    uint32_t tid = engine->next_track_id++;
    engine->tracks[slot].track_id = tid;
    engine->tracks[slot].type = track_type;
    engine->tracks[slot].is_active = true;
    strncpy(engine->tracks[slot].owner_peer_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);
    gafana_mutex_unlock(&engine->track_lock);

    /* Announce track publication */
    uint32_t announce_buf[2];
    announce_buf[0] = (uint32_t)track_type;
    announce_buf[1] = tid;
    gafana_broadcast_packet(engine, GAFANA_OP_TRACK_ANNOUNCE, announce_buf, sizeof(announce_buf));

    if (engine->cbs.on_track_published) {
        engine->cbs.on_track_published(engine, engine->self_peer_id, track_type, tid, engine->user_data);
    }

    *out_track_id = tid;
    return GAFANA_OK;
}

gafana_error_t gafana_track_unpublish(
    gafana_engine_t *engine,
    uint32_t track_id
) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->track_lock);
    gafana_track_type_t ttype = GAFANA_TRACK_AUDIO;
    bool found = false;
    for (int i = 0; i < 64; ++i) {
        if (engine->tracks[i].is_active && engine->tracks[i].track_id == track_id) {
            engine->tracks[i].is_active = false;
            ttype = engine->tracks[i].type;
            found = true;
            break;
        }
    }
    gafana_mutex_unlock(&engine->track_lock);

    if (!found) return GAFANA_ERR_INVALID_PARAM;

    gafana_broadcast_packet(engine, GAFANA_OP_TRACK_REMOVE, &track_id, sizeof(track_id));

    if (engine->cbs.on_track_unpublished) {
        engine->cbs.on_track_unpublished(engine, engine->self_peer_id, ttype, track_id, engine->user_data);
    }

    return GAFANA_OK;
}

gafana_error_t gafana_send_video_frame(
    gafana_engine_t *engine,
    uint32_t track_id,
    const gafana_video_frame_t *frame
) {
    if (!engine || !frame || !frame->data) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    /* Write directly to local file if local recorder is active (ZERO SERVER RETENTION) */
    gafana_mutex_lock(&engine->recorder.lock);
    if (engine->recorder.is_recording && engine->recorder.fp && engine->recorder.rec_type != GAFANA_REC_AUDIO_ONLY) {
        /* Write lightweight binary frame header */
        uint32_t meta[4];
        meta[0] = frame->width;
        meta[1] = frame->height;
        meta[2] = (uint32_t)frame->size;
        meta[3] = frame->timestamp_ms;
        fwrite(meta, sizeof(uint32_t), 4, engine->recorder.fp);
        fwrite(frame->data, 1, frame->size, engine->recorder.fp);
        engine->recorder.bytes_written += (sizeof(uint32_t) * 4) + frame->size;
        engine->recorder.frames_written++;
    }
    gafana_mutex_unlock(&engine->recorder.lock);

    /* Broadcast video frame to active participants */
    gafana_broadcast_packet(engine, GAFANA_OP_MEDIA_VIDEO, frame->data, (uint16_t)(frame->size > 65000 ? 65000 : frame->size));

    return GAFANA_OK;
}

gafana_error_t gafana_send_audio_frame(
    gafana_engine_t *engine,
    uint32_t track_id,
    const gafana_audio_frame_t *frame
) {
    if (!engine || !frame || !frame->pcm_samples) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    size_t byte_count = frame->sample_count * sizeof(int16_t) * frame->channels;

    /* Write directly to local file if recorder is active */
    gafana_mutex_lock(&engine->recorder.lock);
    if (engine->recorder.is_recording && engine->recorder.fp) {
        fwrite(frame->pcm_samples, 1, byte_count, engine->recorder.fp);
        engine->recorder.bytes_written += byte_count;
        engine->recorder.frames_written++;
    }
    gafana_mutex_unlock(&engine->recorder.lock);

    /* Broadcast audio chunk to room */
    gafana_broadcast_packet(engine, GAFANA_OP_MEDIA_AUDIO, frame->pcm_samples, (uint16_t)(byte_count > 65000 ? 65000 : byte_count));

    return GAFANA_OK;
}

gafana_error_t gafana_screen_share_start(gafana_engine_t *engine, uint32_t display_index, uint32_t *out_track_id) {
    (void)display_index;
    return gafana_track_publish(engine, GAFANA_TRACK_SCREEN, out_track_id);
}

gafana_error_t gafana_screen_share_stop(gafana_engine_t *engine, uint32_t track_id) {
    return gafana_track_unpublish(engine, track_id);
}

gafana_error_t gafana_presentation_start(gafana_engine_t *engine, uint32_t total_slides, uint32_t *out_track_id) {
    if (!engine || !out_track_id) return GAFANA_ERR_INVALID_PARAM;
    gafana_error_t err = gafana_track_publish(engine, GAFANA_TRACK_PRESENTATION, out_track_id);
    if (err != GAFANA_OK) return err;

    gafana_mutex_lock(&engine->pres_lock);
    engine->presentation.is_presenting = true;
    engine->presentation.track_id = *out_track_id;
    strncpy(engine->presentation.presenter_id, engine->self_peer_id, GAFANA_MAX_ID_LEN - 1);
    engine->presentation.current_slide = 1;
    engine->presentation.total_slides = total_slides;
    engine->presentation.pointer_x = 0.5f;
    engine->presentation.pointer_y = 0.5f;
    engine->presentation.color_hex = 0xFF0000; /* Red laser */
    gafana_mutex_unlock(&engine->pres_lock);

    gafana_presentation_event_t ev;
    ev.type = GAFANA_PRES_SLIDE_CHANGE;
    ev.current_slide = 1;
    ev.total_slides = total_slides;
    ev.pointer_x = 0.5f;
    ev.pointer_y = 0.5f;
    ev.color_hex = 0xFF0000;
    gafana_presentation_send_event(engine, *out_track_id, &ev);

    return GAFANA_OK;
}

gafana_error_t gafana_presentation_send_event(
    gafana_engine_t *engine,
    uint32_t track_id,
    const gafana_presentation_event_t *event
) {
    if (!engine || !event) return GAFANA_ERR_INVALID_PARAM;
    if (!engine->in_room) return GAFANA_ERR_ROOM_NOT_JOINED;

    gafana_mutex_lock(&engine->pres_lock);
    engine->presentation.current_slide = event->current_slide;
    engine->presentation.pointer_x = event->pointer_x;
    engine->presentation.pointer_y = event->pointer_y;
    engine->presentation.color_hex = event->color_hex;
    gafana_mutex_unlock(&engine->pres_lock);

    gafana_pkt_pres_t pkt;
    pkt.event_type = (uint8_t)event->type;
    pkt.current_slide = event->current_slide;
    pkt.total_slides = event->total_slides;
    pkt.pointer_x = event->pointer_x;
    pkt.pointer_y = event->pointer_y;
    pkt.color_hex = event->color_hex;

    gafana_broadcast_packet(engine, GAFANA_OP_PRES_EVENT, &pkt, sizeof(pkt));

    return GAFANA_OK;
}

gafana_error_t gafana_presentation_stop(gafana_engine_t *engine, uint32_t track_id) {
    if (!engine) return GAFANA_ERR_INVALID_PARAM;

    gafana_mutex_lock(&engine->pres_lock);
    engine->presentation.is_presenting = false;
    gafana_mutex_unlock(&engine->pres_lock);

    gafana_presentation_event_t ev;
    ev.type = GAFANA_PRES_STOP;
    ev.current_slide = 0;
    ev.total_slides = 0;
    ev.pointer_x = 0.0f;
    ev.pointer_y = 0.0f;
    ev.color_hex = 0;

    gafana_pkt_pres_t pkt;
    pkt.event_type = (uint8_t)ev.type;
    pkt.current_slide = 0;
    pkt.total_slides = 0;
    pkt.pointer_x = 0;
    pkt.pointer_y = 0;
    pkt.color_hex = 0;
    gafana_broadcast_packet(engine, GAFANA_OP_PRES_EVENT, &pkt, sizeof(pkt));

    return gafana_track_unpublish(engine, track_id);
}
