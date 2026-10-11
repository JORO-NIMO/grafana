/**
 * Gafana - High-Performance, Privacy-First, Zero-Data Video Calling Engine
 *
 * Production C API Header (v1.2.0)
 *
 * Designed for Desktop (Native C/C++, Qt, Electron, Tauri, Flutter) and Web (WASM/WebRTC).
 * Strictly enforces:
 *  1. Zero Cloud Data: Stateless routing, zero database, zero server disk caching.
 *  2. Unanimous Consensus Recording: 100% agreement required from all room participants.
 *  3. Dynamic Revocation: Mid-call revocation instantly halts local disk capture.
 *  4. Scalable Media Architecture: Multi-track Audio, Video, Screen Share, and Presentations.
 */

#ifndef GAFANA_H
#define GAFANA_H

#ifdef __cplusplus
extern "C" {
#endif

#include <stddef.h>
#include <stdint.h>
#include <stdbool.h>

#define GAFANA_VERSION_MAJOR 1
#define GAFANA_VERSION_MINOR 2
#define GAFANA_VERSION_PATCH 0
#define GAFANA_VERSION_STRING "1.2.0"

#define GAFANA_MAX_ID_LEN         64
#define GAFANA_MAX_NAME_LEN       128
#define GAFANA_MAX_PATH_LEN       512
#define GAFANA_MAX_ROOM_PEERS     4096
#define GAFANA_MAX_TRACKS_PER_PEER 8
#define GAFANA_DEFAULT_PORT       8765
#define GAFANA_BALLOT_TIMEOUT_SEC 30

#if defined(_WIN32) || defined(__CYGWIN__)
  #if defined(GAFANA_BUILD_DLL)
    #define GAFANA_API __declspec(dllexport)
  #elif defined(GAFANA_USE_DLL)
    #define GAFANA_API __declspec(dllimport)
  #else
    #define GAFANA_API
  #endif
#else
  #if __GNUC__ >= 4
    #define GAFANA_API __attribute__((visibility("default")))
  #else
    #define GAFANA_API
  #endif
#endif

/* ========================================================================= */
/* Error Codes                                                               */
/* ========================================================================= */

typedef enum gafana_error_e {
    GAFANA_OK = 0,
    GAFANA_ERR_INVALID_PARAM        = -1,
    GAFANA_ERR_NOT_INITIALIZED      = -2,
    GAFANA_ERR_ALREADY_INITIALIZED  = -3,
    GAFANA_ERR_ROOM_NOT_JOINED      = -4,
    GAFANA_ERR_ALREADY_IN_ROOM      = -5,
    GAFANA_ERR_PEER_NOT_FOUND       = -6,
    GAFANA_ERR_CONSENT_PENDING      = -7,
    GAFANA_ERR_CONSENT_DENIED       = -8,
    GAFANA_ERR_CONSENT_REVOKED      = -9,
    GAFANA_ERR_CONSENT_TIMEOUT      = -10,
    GAFANA_ERR_RECORDING_ACTIVE     = -11,
    GAFANA_ERR_RECORDING_INACTIVE   = -12,
    GAFANA_ERR_UNAUTHORIZED         = -13,
    GAFANA_ERR_NETWORK              = -14,
    GAFANA_ERR_SOCKET               = -15,
    GAFANA_ERR_IO                   = -16,
    GAFANA_ERR_OUT_OF_MEMORY        = -17,
    GAFANA_ERR_BUFFER_OVERFLOW      = -18,
    GAFANA_ERR_UNSUPPORTED          = -19
} gafana_error_t;

/* ========================================================================= */
/* Media Tracks & Recording                                                  */
/* ========================================================================= */

typedef enum gafana_track_type_e {
    GAFANA_TRACK_AUDIO        = 1,
    GAFANA_TRACK_VIDEO        = 2,
    GAFANA_TRACK_SCREEN       = 3,
    GAFANA_TRACK_PRESENTATION = 4
} gafana_track_type_t;

typedef enum gafana_recording_type_e {
    GAFANA_REC_AUDIO_ONLY     = 1,
    GAFANA_REC_FULL_AV        = 2,
    GAFANA_REC_SCREEN_AUDIO   = 3
} gafana_recording_type_t;

typedef enum gafana_consent_state_e {
    GAFANA_CONSENT_IDLE = 0,
    GAFANA_CONSENT_REQUESTED,      /* Ballot in progress across room peers */
    GAFANA_CONSENT_UNANIMOUS_OK,   /* 100% of participants voted YES */
    GAFANA_CONSENT_REJECTED,       /* At least one participant vetoed (NO) */
    GAFANA_CONSENT_REVOKED,        /* A participant revoked consent during recording */
    GAFANA_CONSENT_EXPIRED         /* Ballot timed out before all voted */
} gafana_consent_state_t;

typedef enum gafana_vote_e {
    GAFANA_VOTE_DENY   = 0,
    GAFANA_VOTE_ACCEPT = 1
} gafana_vote_t;

typedef enum gafana_pres_event_type_e {
    GAFANA_PRES_SLIDE_CHANGE = 1,
    GAFANA_PRES_POINTER_MOVE = 2,
    GAFANA_PRES_ANNOTATE     = 3,
    GAFANA_PRES_STOP         = 4
} gafana_pres_event_type_t;

typedef enum gafana_relay_mode_e {
    GAFANA_RELAY_DIRECT_P2P     = 0,
    GAFANA_RELAY_STATELESS_SFU  = 1
} gafana_relay_mode_t;

/* ========================================================================= */
/* Data Structures                                                           */
/* ========================================================================= */

typedef struct gafana_engine_s gafana_engine_t;

typedef struct gafana_peer_info_s {
    char peer_id[GAFANA_MAX_ID_LEN];
    char display_name[GAFANA_MAX_NAME_LEN];
    bool is_speaking;
    bool has_video;
    bool has_screen;
    bool has_presentation;
    bool has_granted_recording;
    uint32_t join_timestamp_sec;
    uint32_t last_heartbeat_ms;
} gafana_peer_info_t;

typedef struct gafana_config_s {
    char relay_host[256];          /* Hostname / IP of stateless SFU relay */
    uint16_t relay_port;           /* Port of stateless SFU relay */
    gafana_relay_mode_t relay_mode;
    uint32_t max_peers;            /* Maximum capacity for room registry */
    uint32_t video_bitrate_kbps;
    uint32_t audio_bitrate_kbps;
    uint32_t ballot_timeout_sec;   /* Default: GAFANA_BALLOT_TIMEOUT_SEC */
    bool enable_e2ee;
    char e2ee_passphrase[128];
} gafana_config_t;

typedef struct gafana_video_frame_s {
    const uint8_t *data;
    size_t size;
    uint32_t width;
    uint32_t height;
    uint32_t stride;
    uint32_t timestamp_ms;
    uint32_t fourcc;               /* e.g. 'VP80', 'H264', 'RAW ' */
    bool is_keyframe;
} gafana_video_frame_t;

typedef struct gafana_audio_frame_s {
    const int16_t *pcm_samples;
    size_t sample_count;
    uint32_t sample_rate;          /* 48000 Hz recommended */
    uint8_t channels;              /* 1 = Mono, 2 = Stereo */
    uint32_t timestamp_ms;
} gafana_audio_frame_t;

typedef struct gafana_presentation_event_s {
    gafana_pres_event_type_t type;
    uint32_t current_slide;
    uint32_t total_slides;
    float pointer_x;               /* Normalized 0.0f - 1.0f */
    float pointer_y;               /* Normalized 0.0f - 1.0f */
    uint32_t color_hex;
} gafana_presentation_event_t;

/* ========================================================================= */
/* Callbacks                                                                 */
/* ========================================================================= */

typedef struct gafana_callbacks_s {
    void (*on_peer_joined)(gafana_engine_t *engine, const gafana_peer_info_t *peer, void *user_data);
    void (*on_peer_left)(gafana_engine_t *engine, const char *peer_id, const char *reason, void *user_data);

    void (*on_track_published)(gafana_engine_t *engine, const char *peer_id, gafana_track_type_t track_type, uint32_t track_id, void *user_data);
    void (*on_track_unpublished)(gafana_engine_t *engine, const char *peer_id, gafana_track_type_t track_type, uint32_t track_id, void *user_data);

    void (*on_video_frame)(gafana_engine_t *engine, const char *peer_id, gafana_track_type_t source, const gafana_video_frame_t *frame, void *user_data);
    void (*on_audio_frame)(gafana_engine_t *engine, const char *peer_id, const gafana_audio_frame_t *frame, void *user_data);

    /* Consent-Based Recording Handshake */
    void (*on_recording_consent_requested)(gafana_engine_t *engine, uint32_t ballot_id, const char *requesting_peer_id, gafana_recording_type_t rec_type, void *user_data);
    void (*on_recording_vote_update)(gafana_engine_t *engine, uint32_t ballot_id, uint32_t votes_granted, uint32_t votes_total, void *user_data);
    void (*on_recording_state_changed)(gafana_engine_t *engine, uint32_t ballot_id, gafana_consent_state_t state, const char *recorder_peer_id, void *user_data);

    /* Presentation Events */
    void (*on_presentation_event)(gafana_engine_t *engine, const char *peer_id, const gafana_presentation_event_t *event, void *user_data);

    /* Connection & Diagnostics */
    void (*on_room_joined)(gafana_engine_t *engine, const char *room_id, const char *self_peer_id, void *user_data);
    void (*on_connection_state)(gafana_engine_t *engine, bool connected, const char *message, void *user_data);
    void (*on_error)(gafana_engine_t *engine, gafana_error_t code, const char *message, void *user_data);
} gafana_callbacks_t;

/* ========================================================================= */
/* Public API Functions                                                      */
/* ========================================================================= */

GAFANA_API const char *gafana_get_version_string(void);
GAFANA_API void gafana_config_default(gafana_config_t *config);

GAFANA_API gafana_error_t gafana_engine_create(
    const gafana_config_t *config,
    const gafana_callbacks_t *callbacks,
    void *user_data,
    gafana_engine_t **out_engine
);

GAFANA_API void gafana_engine_destroy(gafana_engine_t *engine);

GAFANA_API gafana_error_t gafana_room_join(
    gafana_engine_t *engine,
    const char *room_id,
    const char *self_peer_id,
    const char *display_name
);

GAFANA_API gafana_error_t gafana_room_leave(gafana_engine_t *engine);

/**
 * Poll network packets, heartbeats, and check consensus ballot timeouts.
 * Should be called periodically in the application event loop (e.g. every 10-50ms).
 */
GAFANA_API gafana_error_t gafana_engine_poll(gafana_engine_t *engine);

GAFANA_API uint32_t gafana_get_peer_count(gafana_engine_t *engine);

GAFANA_API gafana_error_t gafana_get_peers(
    gafana_engine_t *engine,
    gafana_peer_info_t *out_peers,
    uint32_t max_count,
    uint32_t *out_count
);

/* Media Tracks */
GAFANA_API gafana_error_t gafana_track_publish(gafana_engine_t *engine, gafana_track_type_t track_type, uint32_t *out_track_id);
GAFANA_API gafana_error_t gafana_track_unpublish(gafana_engine_t *engine, uint32_t track_id);
GAFANA_API gafana_error_t gafana_send_video_frame(gafana_engine_t *engine, uint32_t track_id, const gafana_video_frame_t *frame);
GAFANA_API gafana_error_t gafana_send_audio_frame(gafana_engine_t *engine, uint32_t track_id, const gafana_audio_frame_t *frame);

/* Screen Sharing & Presentation */
GAFANA_API gafana_error_t gafana_screen_share_start(gafana_engine_t *engine, uint32_t display_index, uint32_t *out_track_id);
GAFANA_API gafana_error_t gafana_screen_share_stop(gafana_engine_t *engine, uint32_t track_id);
GAFANA_API gafana_error_t gafana_presentation_start(gafana_engine_t *engine, uint32_t total_slides, uint32_t *out_track_id);
GAFANA_API gafana_error_t gafana_presentation_send_event(gafana_engine_t *engine, uint32_t track_id, const gafana_presentation_event_t *event);
GAFANA_API gafana_error_t gafana_presentation_stop(gafana_engine_t *engine, uint32_t track_id);

/* Consensus-Based Local Recording Protocol */
GAFANA_API gafana_error_t gafana_recording_request_consent(gafana_engine_t *engine, gafana_recording_type_t rec_type, uint32_t *out_ballot_id);
GAFANA_API gafana_error_t gafana_recording_cast_vote(gafana_engine_t *engine, uint32_t ballot_id, gafana_vote_t vote);
GAFANA_API gafana_consent_state_t gafana_recording_get_consent_state(gafana_engine_t *engine, uint32_t ballot_id);

/**
 * Start writing media stream directly to the local client disk.
 * Strictly rejected unless 100% unanimous approval is currently active.
 */
GAFANA_API gafana_error_t gafana_recording_start_local(gafana_engine_t *engine, uint32_t ballot_id, const char *output_file_path);
GAFANA_API gafana_error_t gafana_recording_stop_local(gafana_engine_t *engine);
GAFANA_API gafana_error_t gafana_recording_revoke_consent(gafana_engine_t *engine, uint32_t ballot_id);
GAFANA_API bool gafana_recording_is_active(gafana_engine_t *engine);

/* Secure SMTP Email Invitation Service (In-Memory, Zero-Data) */
typedef struct gafana_smtp_config_s {
    char host[128];
    uint16_t port;
    char username[128];
    char password[128];
    char sender_email[128];
    char sender_name[128];
    bool use_tls;
    bool is_configured;
} gafana_smtp_config_t;

GAFANA_API gafana_error_t gafana_smtp_setup(const gafana_smtp_config_t *config);
GAFANA_API gafana_error_t gafana_smtp_get_config(gafana_smtp_config_t *out_config);
GAFANA_API gafana_error_t gafana_smtp_send_invite(
    const char *recipient_email,
    const char *recipient_name,
    const char *room_id,
    const char *meeting_url,
    const char *custom_note,
    char *out_log,
    size_t out_log_sz
);

#ifdef __cplusplus
}
#endif

#endif /* GAFANA_H */
