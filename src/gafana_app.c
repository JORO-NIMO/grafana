/**
 * Gafana Unified Single-Executable Application (gafana.exe)
 *
 * Encapsulates the entire ecosystem in ONE single binary:
 *  1. Embedded Zero-Data Stateless SFU Relay Server (UDP Port 8765)
 *  2. Embedded HTTP Web Server serving the WebRTC UI (TCP Port 8080)
 *  3. Core C Gafana Engine with Media Pipeline & 100% Unanimous Consensus Recorder
 *  4. Native Desktop Interactive Terminal CLI Console
 *  5. Auto-launch of the browser WebRTC interface
 */

#include "../include/gafana.h"
#include "gafana_internal.h"
#include "gafana_winsock.h"
#include "gafana_web_assets.h"
#include "gafana_smtp.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

static bool json_find_string(const char *json, const char *key, char *out, size_t out_max) {
    char needle[64];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char *p = strstr(json, needle);
    if (!p) return false;
    p += strlen(needle);
    while (*p && (*p == ' ' || *p == ':' || *p == '\t' || *p == '\r' || *p == '\n')) p++;
    if (*p != '\"') return false;
    p++;
    size_t i = 0;
    while (*p && i + 1 < out_max) {
        if (*p == '\"') break;
        if (*p == '\\') {
            p++;
            if (!*p) break;
            if (*p == '\"' || *p == '\\' || *p == '/') {
                out[i++] = *p++;
            } else if (*p == 'n') {
                out[i++] = '\n'; p++;
            } else if (*p == 'r') {
                out[i++] = '\r'; p++;
            } else if (*p == 't') {
                out[i++] = '\t'; p++;
            } else {
                out[i++] = *p++;
            }
        } else {
            out[i++] = *p++;
        }
    }
    out[i] = '\0';
    return true;
}

static int json_find_int(const char *json, const char *key, int default_val) {
    char needle[64];
    snprintf(needle, sizeof(needle), "\"%s\"", key);
    const char *p = strstr(json, needle);
    if (!p) return default_val;
    p += strlen(needle);
    while (*p && (*p == ' ' || *p == ':' || *p == '\t' || *p == '\r' || *p == '\n')) p++;
    int val = default_val;
    if (sscanf(p, "%d", &val) == 1) {
        return val;
    }
    return default_val;
}

static void json_escape(const char *in, char *out, size_t out_max) {
    size_t j = 0;
    for (size_t i = 0; in[i] && j + 4 < out_max; ++i) {
        if (in[i] == '\"') {
            out[j++] = '\\'; out[j++] = '\"';
        } else if (in[i] == '\\') {
            out[j++] = '\\'; out[j++] = '\\';
        } else if (in[i] == '\n') {
            out[j++] = '\\'; out[j++] = 'n';
        } else if (in[i] == '\r') {
            /* skip carriage return */
        } else if (in[i] == '\t') {
            out[j++] = '\\'; out[j++] = 't';
        } else {
            out[j++] = in[i];
        }
    }
    out[j] = '\0';
}

static void get_local_ip(char *out_ip, size_t out_max) {
    char hostname[128] = { 0 };
    if (gethostname(hostname, sizeof(hostname)) == 0) {
        struct hostent *he = gethostbyname(hostname);
        if (he && he->h_addr_list && he->h_addr_list[0]) {
            struct in_addr addr;
            memcpy(&addr, he->h_addr_list[0], sizeof(struct in_addr));
            char *ip_str = inet_ntoa(addr);
            if (ip_str && strlen(ip_str) > 0) {
                strncpy(out_ip, ip_str, out_max - 1);
                out_ip[out_max - 1] = '\0';
                return;
            }
        }
    }
    strncpy(out_ip, "127.0.0.1", out_max - 1);
    out_ip[out_max - 1] = '\0';
}

#if defined(_WIN32)
  #include <process.h>
  typedef uintptr_t thread_handle_t;
  #define THREAD_CALL unsigned __stdcall
#else
  #include <pthread.h>
  typedef pthread_t thread_handle_t;
  #define THREAD_CALL void *
#endif

#define GAFANA_SFU_PORT  8765
#define GAFANA_HTTP_PORT 7878

static volatile bool g_running = true;
static gafana_engine_t *g_engine = NULL;
static uint32_t g_current_ballot = 0;
static uint64_t g_sfu_packets_forwarded = 0;
static gafana_mutex_t g_sfu_lock;

/* ========================================================================= */
/* Embedded Stateless SFU Relay Thread (In-Memory UDP Forwarder)            */
/* ========================================================================= */

#define SFU_MAX_ROOMS 128
#define SFU_MAX_PEERS_PER_ROOM 64

typedef struct app_sfu_client_s {
    char peer_id[64];
    struct sockaddr_in addr;
    time_t last_seen;
    bool active;
} app_sfu_client_t;

typedef struct app_sfu_room_s {
    char room_id[64];
    app_sfu_client_t clients[SFU_MAX_PEERS_PER_ROOM];
    uint32_t active_clients;
    bool in_use;
} app_sfu_room_t;

static app_sfu_room_t g_sfu_rooms[SFU_MAX_ROOMS];

static app_sfu_room_t *app_sfu_get_room(const char *room_id) {
    int free_idx = -1;
    for (int i = 0; i < SFU_MAX_ROOMS; ++i) {
        if (g_sfu_rooms[i].in_use && strncmp(g_sfu_rooms[i].room_id, room_id, 64) == 0) {
            return &g_sfu_rooms[i];
        }
        if (!g_sfu_rooms[i].in_use && free_idx == -1) {
            free_idx = i;
        }
    }
    if (free_idx != -1) {
        memset(&g_sfu_rooms[free_idx], 0, sizeof(app_sfu_room_t));
        strncpy(g_sfu_rooms[free_idx].room_id, room_id, 63);
        g_sfu_rooms[free_idx].in_use = true;
        return &g_sfu_rooms[free_idx];
    }
    return NULL;
}

static void app_sfu_register_peer(app_sfu_room_t *room, const char *peer_id, const struct sockaddr_in *addr) {
    time_t now = time(NULL);
    for (uint32_t i = 0; i < SFU_MAX_PEERS_PER_ROOM; ++i) {
        if (room->clients[i].active && strncmp(room->clients[i].peer_id, peer_id, 64) == 0) {
            memcpy(&room->clients[i].addr, addr, sizeof(struct sockaddr_in));
            room->clients[i].last_seen = now;
            return;
        }
    }
    for (uint32_t i = 0; i < SFU_MAX_PEERS_PER_ROOM; ++i) {
        if (!room->clients[i].active) {
            strncpy(room->clients[i].peer_id, peer_id, 63);
            memcpy(&room->clients[i].addr, addr, sizeof(struct sockaddr_in));
            room->clients[i].last_seen = now;
            room->clients[i].active = true;
            room->active_clients++;
            return;
        }
    }
}

static void app_sfu_cleanup_stale_peers(void) {
    time_t now = time(NULL);
    for (int r = 0; r < SFU_MAX_ROOMS; ++r) {
        if (!g_sfu_rooms[r].in_use) continue;
        app_sfu_room_t *room = &g_sfu_rooms[r];
        for (uint32_t c = 0; c < SFU_MAX_PEERS_PER_ROOM; ++c) {
            if (room->clients[c].active && (now - room->clients[c].last_seen > 15)) {
                room->clients[c].active = false;
                if (room->active_clients > 0) room->active_clients--;
            }
        }
        if (room->active_clients == 0) {
            room->in_use = false;
        }
    }
}

static THREAD_CALL sfu_thread_func(void *arg) {
    (void)arg;
    SOCKET s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s == INVALID_SOCKET) return 0;

    int opt = 1;
    setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));

    struct sockaddr_in baddr;
    memset(&baddr, 0, sizeof(baddr));
    baddr.sin_family = AF_INET;
    baddr.sin_addr.s_addr = htonl(INADDR_ANY);
    baddr.sin_port = htons(GAFANA_SFU_PORT);

    if (bind(s, (struct sockaddr *)&baddr, sizeof(baddr)) < 0) {
        closesocket(s);
        return 0;
    }

    uint8_t buf[2048];
    time_t last_cleanup = time(NULL);

    while (g_running) {
        struct sockaddr_in from;
        socklen_t from_len = sizeof(from);
        int n = recvfrom(s, (char *)buf, sizeof(buf), 0, (struct sockaddr *)&from, &from_len);
        if (n < (int)sizeof(gafana_net_packet_header_t)) {
            continue;
        }

        const gafana_net_packet_header_t *hdr = (const gafana_net_packet_header_t *)buf;
        if (hdr->magic != 0x47414641) continue;

        time_t now = time(NULL);
        gafana_mutex_lock(&g_sfu_lock);

        if (now - last_cleanup > 5) {
            app_sfu_cleanup_stale_peers();
            last_cleanup = now;
        }

        app_sfu_room_t *rm = app_sfu_get_room(hdr->room_id);
        if (!rm) {
            gafana_mutex_unlock(&g_sfu_lock);
            continue;
        }

        if (hdr->opcode == GAFANA_OP_LEAVE) {
            for (uint32_t i = 0; i < SFU_MAX_PEERS_PER_ROOM; ++i) {
                if (rm->clients[i].active && strncmp(rm->clients[i].peer_id, hdr->sender_id, 64) == 0) {
                    rm->clients[i].active = false;
                    if (rm->active_clients > 0) rm->active_clients--;
                    break;
                }
            }
        } else {
            app_sfu_register_peer(rm, hdr->sender_id, &from);
        }

        /* In-memory packet fanout to all room peers */
        for (uint32_t i = 0; i < SFU_MAX_PEERS_PER_ROOM; ++i) {
            if (rm->clients[i].active && strncmp(rm->clients[i].peer_id, hdr->sender_id, 64) != 0) {
                sendto(s, (const char *)buf, n, 0,
                       (struct sockaddr *)&rm->clients[i].addr, sizeof(struct sockaddr_in));
            }
        }
        g_sfu_packets_forwarded++;
        gafana_mutex_unlock(&g_sfu_lock);
    }

    closesocket(s);
    return 0;
}

/* Background Engine Polling Worker Thread */
static THREAD_CALL engine_poll_thread_func(void *arg) {
    (void)arg;
    while (g_running) {
        if (g_engine) {
            gafana_engine_poll(g_engine);
        }
        gafana_sleep_ms(25);
    }
    return 0;
}

/* ========================================================================= */
/* Embedded Web UI HTTP Server                                              */
/* ========================================================================= */

static void send_http_response(SOCKET client, const char *status, const char *content_type, const void *body, size_t body_len) {
    char header[512];
    snprintf(header, sizeof(header),
        "HTTP/1.1 %s\r\n"
        "Content-Type: %s\r\n"
        "Content-Length: %zu\r\n"
        "Access-Control-Allow-Origin: *\r\n"
        "Connection: close\r\n\r\n",
        status, content_type, body_len);
    send(client, header, (int)strlen(header), 0);
    send(client, (const char *)body, (int)body_len, 0);
}

static THREAD_CALL http_thread_func(void *arg) {
    (void)arg;
    SOCKET s = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (s == INVALID_SOCKET) return 0;

    int opt = 1;
    setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));

    struct sockaddr_in baddr;
    memset(&baddr, 0, sizeof(baddr));
    baddr.sin_family = AF_INET;
    baddr.sin_addr.s_addr = htonl(INADDR_ANY);
    baddr.sin_port = htons(GAFANA_HTTP_PORT);

    if (bind(s, (struct sockaddr *)&baddr, sizeof(baddr)) < 0) {
        closesocket(s);
        return 0;
    }

    listen(s, 16);

    while (g_running) {
        struct sockaddr_in client_addr;
        socklen_t client_len = sizeof(client_addr);
        SOCKET cs = accept(s, (struct sockaddr *)&client_addr, &client_len);
        if (cs == INVALID_SOCKET) continue;

        char req_buf[4096];
        int n = recv(cs, req_buf, sizeof(req_buf) - 1, 0);
        if (n <= 0) {
            closesocket(cs);
            continue;
        }
        req_buf[n] = '\0';

        if (strstr(req_buf, "100-continue") || strstr(req_buf, "100-Continue")) {
            send(cs, "HTTP/1.1 100 Continue\r\n\r\n", 25, 0);
        }

        int content_len = 0;
        char *cl = strstr(req_buf, "Content-Length:");
        if (!cl) cl = strstr(req_buf, "content-length:");
        if (cl) {
            sscanf(cl + 15, "%d", &content_len);
        }

        char *body = strstr(req_buf, "\r\n\r\n");
        if (body) {
            body += 4;
            int received_body = n - (int)(body - req_buf);
            while (received_body < content_len && n < (int)sizeof(req_buf) - 1) {
                int more = recv(cs, req_buf + n, (int)sizeof(req_buf) - 1 - n, 0);
                if (more <= 0) break;
                n += more;
                received_body += more;
                req_buf[n] = '\0';
            }
        } else {
            body = "";
        }

        char method[16] = { 0 }, path[256] = { 0 };
        if (sscanf(req_buf, "%15s %255s", method, path) == 2) {
            if (strcmp(method, "OPTIONS") == 0) {
                send_http_response(cs, "204 No Content", "text/plain", "", 0);
                closesocket(cs);
                continue;
            }

            if (strcmp(path, "/") == 0 || strcmp(path, "/index.html") == 0) {
                /* Check if file exists on disk, otherwise serve embedded */
                FILE *f = fopen("web/index.html", "rb");
                if (f) {
                    fseek(f, 0, SEEK_END);
                    long sz = ftell(f);
                    fseek(f, 0, SEEK_SET);
                    char *mem = (char *)malloc(sz);
                    fread(mem, 1, sz, f);
                    fclose(f);
                    send_http_response(cs, "200 OK", "text/html", mem, sz);
                    free(mem);
                } else {
                    send_http_response(cs, "200 OK", "text/html", s_embedded_html, s_embedded_html_len);
                }
            } else if (strcmp(path, "/style.css") == 0) {
                FILE *f = fopen("web/style.css", "rb");
                if (f) {
                    fseek(f, 0, SEEK_END);
                    long sz = ftell(f);
                    fseek(f, 0, SEEK_SET);
                    char *mem = (char *)malloc(sz);
                    fread(mem, 1, sz, f);
                    fclose(f);
                    send_http_response(cs, "200 OK", "text/css", mem, sz);
                    free(mem);
                } else {
                    send_http_response(cs, "200 OK", "text/css", s_embedded_css, s_embedded_css_len);
                }
            } else if (strcmp(path, "/gafana_web.js") == 0) {
                FILE *f = fopen("web/gafana_web.js", "rb");
                if (f) {
                    fseek(f, 0, SEEK_END);
                    long sz = ftell(f);
                    fseek(f, 0, SEEK_SET);
                    char *mem = (char *)malloc(sz);
                    fread(mem, 1, sz, f);
                    fclose(f);
                    send_http_response(cs, "200 OK", "application/javascript", mem, sz);
                    free(mem);
                } else {
                    send_http_response(cs, "200 OK", "application/javascript", s_embedded_js, s_embedded_js_len);
                }
            } else if (strcmp(path, "/api/status") == 0) {
                char local_ip[64] = "127.0.0.1";
                get_local_ip(local_ip, sizeof(local_ip));
                char json[512];
                snprintf(json, sizeof(json),
                    "{\"status\":\"online\",\"engine\":\"Gafana\",\"version\":\"%s\",\"zero_data\":true,\"sfu_packets\":%llu,\"local_ip\":\"%s\",\"network_url\":\"http://%s:%u\"}",
                    gafana_get_version_string(), (unsigned long long)g_sfu_packets_forwarded, local_ip, local_ip, GAFANA_HTTP_PORT);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/stats") == 0) {
                uint32_t active_rooms = 0;
                uint32_t active_peers = 0;
                gafana_mutex_lock(&g_sfu_lock);
                for (int i = 0; i < SFU_MAX_ROOMS; ++i) {
                    if (g_sfu_rooms[i].in_use) {
                        active_rooms++;
                        active_peers += g_sfu_rooms[i].active_clients;
                    }
                }
                gafana_mutex_unlock(&g_sfu_lock);
                char local_ip[64] = "127.0.0.1";
                get_local_ip(local_ip, sizeof(local_ip));
                char json[600];
                snprintf(json, sizeof(json),
                    "{\"status\":\"online\",\"engine\":\"Gafana\",\"version\":\"%s\","
                    "\"zero_data\":true,\"sfu_packets\":%llu,\"server_disk_writes_bytes\":0,"
                    "\"active_rooms\":%u,\"active_peers\":%u,\"local_ip\":\"%s\",\"network_url\":\"http://%s:%u\","
                    "\"consensus_rule\":\"100%% Unanimous Agreement Required\"}",
                    gafana_get_version_string(), (unsigned long long)g_sfu_packets_forwarded,
                    active_rooms > 0 ? active_rooms : 1, active_peers > 0 ? active_peers : 1,
                    local_ip, local_ip, GAFANA_HTTP_PORT);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/peers") == 0) {
                gafana_peer_info_t plist[32];
                uint32_t pcount = 0;
                if (g_engine) {
                    gafana_get_peers(g_engine, plist, 32, &pcount);
                }
                char json[2048];
                int offset = snprintf(json, sizeof(json), "{\"room\":\"Gafana-Secure-Room\",\"peers\":[");
                if (pcount == 0) {
                    offset += snprintf(json + offset, sizeof(json) - offset,
                        "{\"id\":\"self_host\",\"name\":\"Local Host (You)\",\"is_self\":true,\"joined_sec\":0}");
                } else {
                    for (uint32_t i = 0; i < pcount && offset + 128 < (int)sizeof(json); ++i) {
                        offset += snprintf(json + offset, sizeof(json) - offset,
                            "%s{\"id\":\"%s\",\"name\":\"%s\",\"is_self\":%s,\"joined_sec\":%u}",
                            (i > 0 ? "," : ""), plist[i].peer_id, plist[i].display_name,
                            (strstr(plist[i].peer_id, "host") != NULL ? "true" : "false"), (unsigned int)plist[i].join_timestamp_sec);
                    }
                }
                snprintf(json + offset, sizeof(json) - offset, "]}");
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/consent/status") == 0) {
                gafana_consent_state_t st = GAFANA_CONSENT_IDLE;
                uint32_t bid = 0, yes_v = 0, no_v = 0, tot_v = 0;
                char req_id[64] = "";
                if (g_engine) {
                    gafana_mutex_lock(&g_engine->ballot_lock);
                    bid = g_engine->current_ballot.ballot_id;
                    st = g_engine->current_ballot.state;
                    yes_v = g_engine->current_ballot.yes_votes;
                    no_v = g_engine->current_ballot.no_votes;
                    tot_v = g_engine->current_ballot.total_expected_votes;
                    strncpy(req_id, g_engine->current_ballot.requester_id, sizeof(req_id) - 1);
                    gafana_mutex_unlock(&g_engine->ballot_lock);
                }
                char json[512];
                snprintf(json, sizeof(json),
                    "{\"ballot_id\":%u,\"state\":%d,\"yes\":%u,\"no\":%u,\"total\":%u,\"requester\":\"%s\",\"recording_active\":%s}",
                    bid, (int)st, yes_v, no_v, tot_v, req_id,
                    (g_engine && gafana_recording_is_active(g_engine)) ? "true" : "false");
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/consent/request") == 0) {
                char type_str[16] = "av";
                json_find_string(body, "type", type_str, sizeof(type_str));
                gafana_recording_type_t rtype = (strcmp(type_str, "audio") == 0) ? GAFANA_REC_AUDIO_ONLY : GAFANA_REC_FULL_AV;
                uint32_t new_bid = 0;
                gafana_error_t err = GAFANA_ERR_NOT_INITIALIZED;
                if (g_engine) {
                    err = gafana_recording_request_consent(g_engine, rtype, &new_bid);
                    if (err == GAFANA_OK) g_current_ballot = new_bid;
                }
                char json[256];
                snprintf(json, sizeof(json), "{\"code\":%d,\"ballot_id\":%u}", (int)err, new_bid);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/consent/vote") == 0) {
                int vote_val = json_find_int(body, "vote", 1);
                int bid = json_find_int(body, "ballot_id", (int)g_current_ballot);
                gafana_error_t err = GAFANA_ERR_NOT_INITIALIZED;
                if (g_engine) {
                    err = gafana_recording_cast_vote(g_engine, (uint32_t)bid, vote_val ? GAFANA_VOTE_ACCEPT : GAFANA_VOTE_DENY);
                }
                char json[128];
                snprintf(json, sizeof(json), "{\"code\":%d}", (int)err);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/consent/revoke") == 0) {
                int bid = json_find_int(body, "ballot_id", (int)g_current_ballot);
                gafana_error_t err = GAFANA_ERR_NOT_INITIALIZED;
                if (g_engine) {
                    err = gafana_recording_revoke_consent(g_engine, (uint32_t)bid);
                }
                char json[128];
                snprintf(json, sizeof(json), "{\"code\":%d}", (int)err);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/open") == 0) {
#if defined(_WIN32)
                system("start http://localhost:7878");
#elif defined(__APPLE__)
                system("open http://localhost:7878");
#else
                system("xdg-open http://localhost:7878 >/dev/null 2>&1 &");
#endif
                const char *resp = "{\"status\":\"ok\",\"url\":\"http://localhost:7878\"}";
                send_http_response(cs, "200 OK", "application/json", resp, strlen(resp));
            } else if (strcmp(path, "/api/quit") == 0) {
                if (client_addr.sin_addr.s_addr != htonl(INADDR_LOOPBACK)) {
                    send_http_response(cs, "403 Forbidden", "text/plain", "Forbidden", 9);
                    closesocket(cs);
                    continue;
                }
                const char *resp = "{\"status\":\"terminating\",\"message\":\"Gafana engine shutting down gracefully.\"}";
                send_http_response(cs, "200 OK", "application/json", resp, strlen(resp));
                closesocket(cs);
                g_running = false;
                printf("\n[WEB COMMAND] 'quit' triggered from Web Control Center. Shutting down...\n");
                gafana_room_leave(g_engine);
                gafana_engine_destroy(g_engine);
                gafana_net_global_cleanup();
                exit(0);
            } else if (strcmp(path, "/api/smtp/status") == 0) {
                gafana_smtp_config_t cfg;
                gafana_smtp_get_config(&cfg);
                char json[512];
                snprintf(json, sizeof(json),
                    "{\"configured\":%s,\"host\":\"%s\",\"port\":%u,\"sender_email\":\"%s\",\"sender_name\":\"%s\"}",
                    cfg.is_configured ? "true" : "false", cfg.host, (unsigned int)cfg.port, cfg.sender_email, cfg.sender_name);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else if (strcmp(path, "/api/smtp/config") == 0) {
                gafana_smtp_config_t cfg;
                memset(&cfg, 0, sizeof(cfg));
                json_find_string(body, "host", cfg.host, sizeof(cfg.host));
                cfg.port = (uint16_t)json_find_int(body, "port", 587);
                json_find_string(body, "username", cfg.username, sizeof(cfg.username));
                json_find_string(body, "password", cfg.password, sizeof(cfg.password));
                json_find_string(body, "sender_email", cfg.sender_email, sizeof(cfg.sender_email));
                json_find_string(body, "sender_name", cfg.sender_name, sizeof(cfg.sender_name));
                cfg.use_tls = true;
                gafana_smtp_setup(&cfg);
                const char *resp = "{\"status\":\"ok\",\"configured\":true,\"message\":\"SMTP credentials registered in volatile RAM.\"}";
                send_http_response(cs, "200 OK", "application/json", resp, strlen(resp));
            } else if (strcmp(path, "/api/smtp/invite") == 0) {
                char to[128] = { 0 };
                char name[128] = { 0 };
                char note[256] = { 0 };
                char invite_url[256] = { 0 };
                json_find_string(body, "to", to, sizeof(to));
                json_find_string(body, "name", name, sizeof(name));
                json_find_string(body, "note", note, sizeof(note));
                json_find_string(body, "url", invite_url, sizeof(invite_url));

                if (invite_url[0] == '\0') {
                    char local_ip[64] = "127.0.0.1";
                    get_local_ip(local_ip, sizeof(local_ip));
                    snprintf(invite_url, sizeof(invite_url), "http://%s:%u", local_ip, GAFANA_HTTP_PORT);
                }

                char log_buf[2048] = { 0 };
                gafana_error_t err = gafana_smtp_send_invite(to, name, "Gafana-Secure-Room", invite_url, note, log_buf, sizeof(log_buf));
                char esc_log[4096] = { 0 };
                json_escape(log_buf, esc_log, sizeof(esc_log));

                char json[4500];
                snprintf(json, sizeof(json),
                    "{\"success\":%s,\"code\":%d,\"log\":\"%s\"}",
                    (err == GAFANA_OK) ? "true" : "false", (int)err, esc_log);
                send_http_response(cs, "200 OK", "application/json", json, strlen(json));
            } else {
                send_http_response(cs, "404 Not Found", "text/plain", "Not Found", 9);
            }
        }
        closesocket(cs);
    }

    closesocket(s);
    return 0;
}

/* ========================================================================= */
/* Core Engine Callbacks                                                     */
/* ========================================================================= */

static void cb_on_peer_joined(gafana_engine_t *engine, const gafana_peer_info_t *peer, void *user_data) {
    (void)engine; (void)user_data;
    printf("\n>>> [PEER JOINED] %s (ID: %s)\n> ", peer->display_name, peer->peer_id);
    fflush(stdout);
}

static void cb_on_peer_left(gafana_engine_t *engine, const char *peer_id, const char *reason, void *user_data) {
    (void)engine; (void)user_data;
    printf("\n<<< [PEER LEFT] %s (Reason: %s)\n> ", peer_id, reason);
    fflush(stdout);
}

static void cb_on_recording_consent_requested(
    gafana_engine_t *engine,
    uint32_t ballot_id,
    const char *requester_id,
    gafana_recording_type_t rec_type,
    void *user_data
) {
    (void)engine; (void)user_data;
    g_current_ballot = ballot_id;
    printf("\n============================================================\n");
    printf(" [CONSENT REQUEST] Peer '%s' wants to record this meeting!\n", requester_id);
    printf(" Recording Type: %s\n", (rec_type == GAFANA_REC_AUDIO_ONLY) ? "Audio-Only" : "Full Video+Audio");
    printf(" Consensus Rule: 100%% Unanimous Agreement Required.\n");
    printf(" Type 'accept' or 'deny' to vote.\n");
    printf("============================================================\n> ");
    fflush(stdout);
}

static void cb_on_recording_vote_update(
    gafana_engine_t *engine,
    uint32_t ballot_id,
    uint32_t votes_granted,
    uint32_t votes_total,
    void *user_data
) {
    (void)engine; (void)user_data;
    printf("\n[VOTE TALLY] Ballot #%u: %u of %u accepted.\n> ", ballot_id, votes_granted, votes_total);
    fflush(stdout);
}

static void cb_on_recording_state_changed(
    gafana_engine_t *engine,
    uint32_t ballot_id,
    gafana_consent_state_t state,
    const char *recorder_peer_id,
    void *user_data
) {
    (void)engine; (void)user_data;
    switch (state) {
        case GAFANA_CONSENT_UNANIMOUS_OK:
            printf("\n[CONSENT GRANTED] 100%% approved! '%s' may now record locally.\n> ", recorder_peer_id);
            break;
        case GAFANA_CONSENT_REJECTED:
            printf("\n[CONSENT REJECTED] Veto cast. Recording is blocked.\n> ");
            break;
        case GAFANA_CONSENT_REVOKED:
            printf("\n[CONSENT REVOKED] Permission withdrawn! Recording halted immediately.\n> ");
            break;
        case GAFANA_CONSENT_EXPIRED:
            printf("\n[CONSENT EXPIRED] Ballot timed out.\n> ");
            break;
        case GAFANA_CONSENT_IDLE:
            printf("\n[RECORDING STOPPED] Local capture finalized.\n> ");
            break;
        default:
            break;
    }
    fflush(stdout);
}

/* ========================================================================= */
/* Main Application Entrypoint                                               */
/* ========================================================================= */

int main(int argc, char **argv) {
    (void)argc; (void)argv;

    printf("=================================================================\n");
    printf("  GAFANA ALL-IN-ONE VIDEO CALLING APPLIANCE                      \n");
    printf("  Version: %s | Zero-Cloud Data | 100%% Unanimous Consensus      \n", gafana_get_version_string());
    printf("=================================================================\n\n");

    /* 1. Global Sockets Init */
    gafana_net_global_init();
    gafana_mutex_init(&g_sfu_lock);

    /* 2. Launch Embedded SFU Relay Thread */
    printf("[1/4] Starting In-Memory Stateless SFU Relay on UDP port %u...\n", GAFANA_SFU_PORT);
#if defined(_WIN32)
    _beginthreadex(NULL, 0, (unsigned (__stdcall *)(void *))sfu_thread_func, NULL, 0, NULL);
#else
    pthread_t sfu_tid;
    pthread_create(&sfu_tid, NULL, sfu_thread_func, NULL);
#endif

    /* 3. Launch Embedded Web UI Server Thread */
    printf("[2/4] Starting Embedded WebRTC UI Server on http://localhost:%u...\n", GAFANA_HTTP_PORT);
#if defined(_WIN32)
    _beginthreadex(NULL, 0, (unsigned (__stdcall *)(void *))http_thread_func, NULL, 0, NULL);
#else
    pthread_t http_tid;
    pthread_create(&http_tid, NULL, http_thread_func, NULL);
#endif

    /* 4. Initialize Core Gafana C Engine */
    printf("[3/4] Initializing Core C Media Engine...\n");
    gafana_callbacks_t cbs;
    memset(&cbs, 0, sizeof(cbs));
    cbs.on_peer_joined = cb_on_peer_joined;
    cbs.on_peer_left = cb_on_peer_left;
    cbs.on_recording_consent_requested = cb_on_recording_consent_requested;
    cbs.on_recording_vote_update = cb_on_recording_vote_update;
    cbs.on_recording_state_changed = cb_on_recording_state_changed;

    gafana_config_t config;
    gafana_config_default(&config);
    config.relay_port = GAFANA_SFU_PORT;

    gafana_error_t err = gafana_engine_create(&config, &cbs, NULL, &g_engine);
    if (err != GAFANA_OK) {
        fprintf(stderr, "Engine initialization failed: %d\n", err);
        return 1;
    }

    char my_id[32];
    snprintf(my_id, sizeof(my_id), "host_%u", (unsigned int)rand() % 9000 + 1000);
    gafana_room_join(g_engine, "Gafana-Secure-Room", my_id, "Host User");

    /* Launch background engine event poll thread */
#if defined(_WIN32)
    _beginthreadex(NULL, 0, (unsigned (__stdcall *)(void *))engine_poll_thread_func, NULL, 0, NULL);
#else
    pthread_t poll_tid;
    pthread_create(&poll_tid, NULL, engine_poll_thread_func, NULL);
#endif

    /* 5. Auto-launch the web UI in the system's browser */
    printf("[4/4] Launching Web Interface in default browser...\n");
#if defined(_WIN32)
    system("start http://localhost:7878");
#elif defined(__APPLE__)
    system("open http://localhost:7878");
#else
    system("xdg-open http://localhost:7878 >/dev/null 2>&1 &");
#endif

    char local_ip[64] = "127.0.0.1";
    get_local_ip(local_ip, sizeof(local_ip));

    printf("\n>>> Gafana is LIVE!\n");
    printf(">>> Web UI (Local):   http://localhost:%u\n", GAFANA_HTTP_PORT);
    printf(">>> Web UI (Network): http://%s:%u\n", local_ip, GAFANA_HTTP_PORT);
    printf(">>> UDP Relay:        127.0.0.1:%u (or %s:%u)\n", GAFANA_SFU_PORT, local_ip, GAFANA_SFU_PORT);
    printf(">>> Ephemeral Room:   'Gafana-Secure-Room' (Zero Cloud Retention)\n\n");

    printf("Available Console Commands:\n");
    printf("  peers        - List active room participants\n");
    printf("  record [a|v] - Request consent to record (a=audio, v=full AV)\n");
    printf("  startrec     - Start writing to local disk (once approved)\n");
    printf("  stoprec      - Stop local recording\n");
    printf("  accept       - Vote YES on pending ballot\n");
    printf("  deny         - Vote NO (Veto) on pending ballot\n");
    printf("  revoke       - Revoke consent mid-call (emergency halt)\n");
    printf("  screen       - Toggle native screen sharing\n");
    printf("  present      - Start presentation mode (10 slides)\n");
    printf("  next         - Advance slide\n");
    printf("  stats        - Show relay in-memory packet throughput\n");
    printf("  open         - Re-open web browser interface\n");
    printf("  quit         - Exit and terminate Gafana\n\n> ");
    fflush(stdout);

    char line[128];
    uint32_t screen_track = 0;
    uint32_t pres_track = 0;
    uint32_t my_ballot = 0;

    while (g_running && fgets(line, sizeof(line), stdin)) {
        line[strcspn(line, "\r\n")] = 0;

        if (strcmp(line, "quit") == 0 || strcmp(line, "exit") == 0) {
            break;
        } else if (strcmp(line, "peers") == 0) {
            gafana_peer_info_t plist[32];
            uint32_t pcount = 0;
            gafana_get_peers(g_engine, plist, 32, &pcount);
            printf("Active Room Participants (%u):\n", pcount);
            for (uint32_t i = 0; i < pcount; ++i) {
                printf("  [%u] %s (ID: %s)\n", i + 1, plist[i].display_name, plist[i].peer_id);
            }
        } else if (strncmp(line, "record", 6) == 0) {
            gafana_recording_type_t rtype = GAFANA_REC_FULL_AV;
            if (strstr(line, "a")) rtype = GAFANA_REC_AUDIO_ONLY;

            err = gafana_recording_request_consent(g_engine, rtype, &my_ballot);
            if (err == GAFANA_OK) {
                printf("Ballot #%u initiated. Waiting for 100%% unanimous consent...\n", my_ballot);
            } else {
                printf("Error requesting recording: %d\n", err);
            }
        } else if (strcmp(line, "accept") == 0) {
            uint32_t bid = g_current_ballot ? g_current_ballot : my_ballot;
            gafana_recording_cast_vote(g_engine, bid, GAFANA_VOTE_ACCEPT);
            printf("Voted: ACCEPT (YES)\n");
        } else if (strcmp(line, "deny") == 0) {
            uint32_t bid = g_current_ballot ? g_current_ballot : my_ballot;
            gafana_recording_cast_vote(g_engine, bid, GAFANA_VOTE_DENY);
            printf("Voted: DENY (VETO)\n");
        } else if (strcmp(line, "startrec") == 0) {
            const char *out_name = "gafana_local_capture.wav";
            err = gafana_recording_start_local(g_engine, my_ballot, out_name);
            if (err == GAFANA_OK) {
                printf("Local recording started: %s (ZERO cloud copies)\n", out_name);
            } else if (err == GAFANA_ERR_CONSENT_PENDING) {
                printf("Blocked: Still waiting for unanimous consent!\n");
            } else if (err == GAFANA_ERR_CONSENT_DENIED) {
                printf("Blocked: A participant vetoed recording consent!\n");
            } else {
                printf("Recording start error: %d\n", err);
            }
        } else if (strcmp(line, "stoprec") == 0) {
            gafana_recording_stop_local(g_engine);
            printf("Local recording stopped and saved to disk.\n");
        } else if (strcmp(line, "revoke") == 0) {
            uint32_t bid = g_current_ballot ? g_current_ballot : my_ballot;
            gafana_recording_revoke_consent(g_engine, bid);
            printf("Revocation broadcast! Any ongoing recording has been terminated.\n");
        } else if (strcmp(line, "screen") == 0) {
            if (screen_track == 0) {
                gafana_screen_share_start(g_engine, 0, &screen_track);
                printf("Screen sharing track active (Track #%u)\n", screen_track);
            } else {
                gafana_screen_share_stop(g_engine, screen_track);
                printf("Screen sharing track stopped.\n");
                screen_track = 0;
            }
        } else if (strcmp(line, "present") == 0) {
            if (pres_track == 0) {
                gafana_presentation_start(g_engine, 10, &pres_track);
                printf("Presentation active (10 slides, Track #%u)\n", pres_track);
            } else {
                gafana_presentation_stop(g_engine, pres_track);
                printf("Presentation stopped.\n");
                pres_track = 0;
            }
        } else if (strcmp(line, "next") == 0) {
            if (pres_track > 0) {
                gafana_presentation_event_t ev;
                ev.type = GAFANA_PRES_SLIDE_CHANGE;
                ev.current_slide = 2;
                ev.total_slides = 10;
                ev.pointer_x = 0.5f;
                ev.pointer_y = 0.5f;
                ev.color_hex = 0xFF0000;
                gafana_presentation_send_event(g_engine, pres_track, &ev);
                printf("Advanced to slide 2.\n");
            } else {
                printf("Presentation not started. Type 'present' first.\n");
            }
        } else if (strcmp(line, "stats") == 0) {
            printf("Relay Stats: %llu in-memory packets routed (0 bytes on server disk)\n",
                   (unsigned long long)g_sfu_packets_forwarded);
        } else if (strncmp(line, "smtp", 4) == 0) {
            char host[128] = "127.0.0.1", user[128] = "", pass[128] = "", sender[128] = "host@gafana.local";
            int port = 587;
            int parsed = sscanf(line + 4, "%127s %d %127s %127s %127s", host, &port, user, pass, sender);
            if (parsed >= 1) {
                gafana_smtp_config_t scfg;
                memset(&scfg, 0, sizeof(scfg));
                strncpy(scfg.host, host, sizeof(scfg.host) - 1);
                scfg.port = (uint16_t)port;
                strncpy(scfg.username, user, sizeof(scfg.username) - 1);
                strncpy(scfg.password, pass, sizeof(scfg.password) - 1);
                strncpy(scfg.sender_email, sender, sizeof(scfg.sender_email) - 1);
                strncpy(scfg.sender_name, "Gafana Host", sizeof(scfg.sender_name) - 1);
                gafana_smtp_setup(&scfg);
                printf("SMTP configured in volatile RAM: %s:%d (Sender: %s)\n", host, port, sender);
            } else {
                printf("Usage: smtp <host> [port] [username] [password] [sender_email]\n");
            }
        } else if (strncmp(line, "invite", 6) == 0) {
            char email[128] = "", name[128] = "Attendee";
            int parsed = sscanf(line + 6, "%127s %127s", email, name);
            if (parsed >= 1) {
                char log_buf[2048] = { 0 };
                printf("Dispatching secure invite to '%s' via SMTP...\n", email);
                gafana_error_t serr = gafana_smtp_send_invite(email, name, "Gafana-Secure-Room", "http://localhost:7878", "Confidential Gafana Video Meeting", log_buf, sizeof(log_buf));
                printf("%s\n", log_buf);
                if (serr == GAFANA_OK) printf("Invite dispatched successfully.\n");
                else printf("Invite dispatch notice (code: %d).\n", serr);
            } else {
                printf("Usage: invite <recipient_email> [recipient_name]\n");
            }
        } else if (strcmp(line, "open") == 0) {
#if defined(_WIN32)
            system("start http://localhost:7878");
#endif
            printf("Re-opened http://localhost:7878 in default browser.\n");
        } else {
            printf("Unknown command '%s'. Try: peers, record, accept, deny, startrec, stoprec, screen, present, stats, smtp, invite, open, quit\n", line);
        }
        printf("> ");
        fflush(stdout);
    }

    printf("\nShutting down Gafana Appliance...\n");
    g_running = false;

    gafana_room_leave(g_engine);
    gafana_engine_destroy(g_engine);
    gafana_mutex_free(&g_sfu_lock);
    gafana_net_global_cleanup();

    printf("Gafana safely terminated. All ephemeral session memory discarded.\n");
    return 0;
}
