/**
 * Gafana Internal Header - Private engine structures and networking definitions
 */

#ifndef GAFANA_INTERNAL_H
#define GAFANA_INTERNAL_H

#include "../include/gafana.h"
#include "gafana_net.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#if defined(_WIN32)
  #ifndef WIN32_LEAN_AND_MEAN
    #define WIN32_LEAN_AND_MEAN
  #endif
  #include <windows.h>
  typedef CRITICAL_SECTION gafana_mutex_t;
  #define gafana_mutex_init(m)   InitializeCriticalSection(m)
  #define gafana_mutex_lock(m)   EnterCriticalSection(m)
  #define gafana_mutex_unlock(m) LeaveCriticalSection(m)
  #define gafana_mutex_free(m)   DeleteCriticalSection(m)
  #define gafana_sleep_ms(ms)    Sleep(ms)
#else
  #include <pthread.h>
  #include <unistd.h>
  typedef pthread_mutex_t gafana_mutex_t;
  #define gafana_mutex_init(m)   pthread_mutex_init(m, NULL)
  #define gafana_mutex_lock(m)   pthread_mutex_lock(m)
  #define gafana_mutex_unlock(m) pthread_mutex_unlock(m)
  #define gafana_mutex_free(m)   pthread_mutex_destroy(m)
  #define gafana_sleep_ms(ms)    usleep((ms) * 1000)
#endif

#define GAFANA_MAGIC 0x47414641 /* "GAFA" */

#define GAFANA_OP_JOIN              0x01
#define GAFANA_OP_LEAVE             0x02
#define GAFANA_OP_TRACK_ANNOUNCE    0x03
#define GAFANA_OP_TRACK_REMOVE      0x04
#define GAFANA_OP_CONSENT_REQUEST   0x10
#define GAFANA_OP_CONSENT_VOTE      0x11
#define GAFANA_OP_CONSENT_DECISION  0x12
#define GAFANA_OP_CONSENT_REVOKE    0x13
#define GAFANA_OP_MEDIA_VIDEO       0x20
#define GAFANA_OP_MEDIA_AUDIO       0x21
#define GAFANA_OP_PRES_EVENT        0x30
#define GAFANA_OP_HEARTBEAT         0x40

#pragma pack(push, 1)
typedef struct gafana_pkt_header_s {
    uint32_t magic;
    uint8_t  opcode;
    uint8_t  flags;
    uint16_t payload_len;
    uint32_t seq_num;
    uint32_t timestamp;
    char     sender_id[GAFANA_MAX_ID_LEN];
} gafana_pkt_header_t;

typedef struct gafana_pkt_consent_req_s {
    uint32_t ballot_id;
    uint8_t  rec_type;
    uint32_t timeout_sec;
    char     requester_id[GAFANA_MAX_ID_LEN];
} gafana_pkt_consent_req_t;

typedef struct gafana_pkt_consent_vote_s {
    uint32_t ballot_id;
    uint8_t  vote; /* 1 = YES, 0 = NO */
    char     voter_id[GAFANA_MAX_ID_LEN];
} gafana_pkt_consent_vote_t;

typedef struct gafana_pkt_consent_decision_s {
    uint32_t ballot_id;
    uint8_t  decision_state; /* gafana_consent_state_t */
    char     recorder_id[GAFANA_MAX_ID_LEN];
} gafana_pkt_consent_decision_t;

typedef struct gafana_pkt_consent_revoke_s {
    uint32_t ballot_id;
    char     revoker_id[GAFANA_MAX_ID_LEN];
} gafana_pkt_consent_revoke_t;

typedef struct gafana_pkt_pres_s {
    uint8_t  event_type;
    uint32_t current_slide;
    uint32_t total_slides;
    float    pointer_x;
    float    pointer_y;
    uint32_t color_hex;
} gafana_pkt_pres_t;
#pragma pack(pop)

typedef struct gafana_peer_node_s {
    gafana_peer_info_t info;
    bool active;
    uint32_t last_seen_ms;
    gafana_vote_t vote_status;
    bool has_voted;
} gafana_peer_node_t;

typedef struct gafana_track_node_s {
    uint32_t track_id;
    gafana_track_type_t type;
    bool is_active;
    char owner_peer_id[GAFANA_MAX_ID_LEN];
} gafana_track_node_t;

typedef struct gafana_ballot_s {
    uint32_t ballot_id;
    char requester_id[GAFANA_MAX_ID_LEN];
    gafana_recording_type_t rec_type;
    gafana_consent_state_t state;
    uint32_t total_expected_votes;
    uint32_t yes_votes;
    uint32_t no_votes;
    bool is_active;
    uint32_t start_time_ms;
    uint32_t expires_at_ms;
} gafana_ballot_t;

typedef struct gafana_local_recorder_s {
    bool is_recording;
    uint32_t ballot_id;
    gafana_recording_type_t rec_type;
    char file_path[GAFANA_MAX_PATH_LEN];
    FILE *fp;
    uint64_t bytes_written;
    uint32_t frames_written;
    uint32_t start_timestamp_ms;
    gafana_mutex_t lock;
} gafana_local_recorder_t;

typedef struct gafana_presentation_state_s {
    bool is_presenting;
    uint32_t track_id;
    char presenter_id[GAFANA_MAX_ID_LEN];
    uint32_t current_slide;
    uint32_t total_slides;
    float pointer_x;
    float pointer_y;
    uint32_t color_hex;
} gafana_presentation_state_t;

struct gafana_engine_s {
    gafana_config_t config;
    gafana_callbacks_t cbs;
    void *user_data;

    char current_room_id[GAFANA_MAX_ID_LEN];
    char self_peer_id[GAFANA_MAX_ID_LEN];
    char self_display_name[GAFANA_MAX_NAME_LEN];
    bool in_room;

    /* Socket Transport */
    gafana_net_transport_t net;

    /* Peer Table */
    gafana_peer_node_t *peers;
    uint32_t peer_capacity;
    uint32_t active_peer_count;
    gafana_mutex_t peer_lock;

    /* Media Tracks */
    gafana_track_node_t tracks[64];
    uint32_t next_track_id;
    gafana_mutex_t track_lock;

    /* Consensus & Balloting */
    gafana_ballot_t current_ballot;
    uint32_t next_ballot_id;
    gafana_mutex_t ballot_lock;

    /* Local File Recorder */
    gafana_local_recorder_t recorder;

    /* Presentation */
    gafana_presentation_state_t presentation;
    gafana_mutex_t pres_lock;

    /* Heartbeat timer */
    uint32_t last_heartbeat_sent_ms;

    uint32_t packet_seq;
    bool is_running;
};

uint32_t gafana_get_time_ms(void);
gafana_error_t gafana_broadcast_packet(gafana_engine_t *engine, uint8_t opcode, const void *payload, uint16_t payload_len);
gafana_error_t gafana_handle_packet(gafana_engine_t *engine, const uint8_t *buffer, size_t length);
void gafana_consensus_reset(gafana_engine_t *engine);
void gafana_consensus_check_timeouts(gafana_engine_t *engine);

#endif /* GAFANA_INTERNAL_H */
