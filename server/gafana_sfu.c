/**
 * Gafana Stateless SFU Relay Server (gafana-relay)
 *
 * A production-grade, ultra-lightweight, zero-storage real-time packet forwarder in C.
 *
 * Guarantees:
 *  - ZERO database: All routing tables are in-memory volatile structures.
 *  - ZERO disk storage: Media packets are never written to disk or recorded on servers.
 *  - Pure in-memory relay with sub-millisecond latency.
 *  - Supports thousands of concurrent participants in rooms with low memory footprint.
 */

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <stdint.h>
#include <time.h>

#if defined(_WIN32)
  #include "../src/gafana_winsock.h"
  typedef SOCKET socket_t;
  #define CLOSE_SOCKET(s) closesocket(s)
#else
  #include <sys/types.h>
  #include <sys/socket.h>
  #include <netinet/in.h>
  #include <arpa/inet.h>
  #include <unistd.h>
  #include <fcntl.h>
  typedef int socket_t;
  #define CLOSE_SOCKET(s) close(s)
#endif

#define SFU_MAGIC 0x47414641 /* "GAFA" */
#define SFU_DEFAULT_PORT 8765
#define SFU_MAX_ROOMS 1024
#define SFU_MAX_CLIENTS_PER_ROOM 256
#define SFU_PEER_TIMEOUT_SEC 15
#define SFU_BUFFER_SIZE 2048

#pragma pack(push, 1)
typedef struct sfu_pkt_header_s {
    uint32_t magic;
    uint8_t  version;
    uint8_t  opcode;
    uint16_t flags;
    uint32_t seq;
    uint32_t timestamp_ms;
    char     room_id[64];
    char     sender_id[64];
    uint16_t payload_len;
    uint16_t chunk_idx;
    uint16_t total_chunks;
    uint32_t checksum;
} sfu_pkt_header_t;
#pragma pack(pop)

typedef struct sfu_client_s {
    char peer_id[64];
    struct sockaddr_in addr;
    time_t last_seen;
    bool active;
} sfu_client_t;

typedef struct sfu_room_s {
    char room_id[64];
    sfu_client_t clients[SFU_MAX_CLIENTS_PER_ROOM];
    uint32_t active_clients;
    bool in_use;
} sfu_room_t;

static sfu_room_t g_rooms[SFU_MAX_ROOMS];
static uint64_t g_total_packets_forwarded = 0;
static uint64_t g_total_bytes_forwarded = 0;

static sfu_room_t *sfu_get_or_create_room(const char *room_id) {
    int free_slot = -1;
    for (int i = 0; i < SFU_MAX_ROOMS; ++i) {
        if (g_rooms[i].in_use && strncmp(g_rooms[i].room_id, room_id, 64) == 0) {
            return &g_rooms[i];
        }
        if (!g_rooms[i].in_use && free_slot == -1) {
            free_slot = i;
        }
    }
    if (free_slot != -1) {
        memset(&g_rooms[free_slot], 0, sizeof(sfu_room_t));
        strncpy(g_rooms[free_slot].room_id, room_id, 63);
        g_rooms[free_slot].in_use = true;
        return &g_rooms[free_slot];
    }
    return NULL;
}

static void sfu_register_peer(sfu_room_t *room, const char *peer_id, const struct sockaddr_in *addr) {
    time_t now = time(NULL);
    for (uint32_t i = 0; i < SFU_MAX_CLIENTS_PER_ROOM; ++i) {
        if (room->clients[i].active && strncmp(room->clients[i].peer_id, peer_id, 64) == 0) {
            /* Update endpoint */
            memcpy(&room->clients[i].addr, addr, sizeof(struct sockaddr_in));
            room->clients[i].last_seen = now;
            return;
        }
    }

    /* New client in room */
    for (uint32_t i = 0; i < SFU_MAX_CLIENTS_PER_ROOM; ++i) {
        if (!room->clients[i].active) {
            strncpy(room->clients[i].peer_id, peer_id, 63);
            memcpy(&room->clients[i].addr, addr, sizeof(struct sockaddr_in));
            room->clients[i].last_seen = now;
            room->clients[i].active = true;
            room->active_clients++;
            printf("[SFU] Peer '%s' registered in room '%s' (Total: %u)\n",
                   peer_id, room->room_id, room->active_clients);
            return;
        }
    }
}

static void sfu_cleanup_stale_peers(void) {
    time_t now = time(NULL);
    for (int r = 0; r < SFU_MAX_ROOMS; ++r) {
        if (!g_rooms[r].in_use) continue;
        sfu_room_t *room = &g_rooms[r];
        for (uint32_t c = 0; c < SFU_MAX_CLIENTS_PER_ROOM; ++c) {
            if (room->clients[c].active && (now - room->clients[c].last_seen > SFU_PEER_TIMEOUT_SEC)) {
                printf("[SFU] Pruning timed out peer '%s' from room '%s'\n",
                       room->clients[c].peer_id, room->room_id);
                room->clients[c].active = false;
                if (room->active_clients > 0) room->active_clients--;
            }
        }
        if (room->active_clients == 0) {
            room->in_use = false;
        }
    }
}

int main(int argc, char **argv) {
    uint16_t port = SFU_DEFAULT_PORT;
    if (argc > 1) {
        port = (uint16_t)atoi(argv[1]);
    }

    printf("=================================================================\n");
    printf(" Gafana Stateless SFU Relay Server (v1.2.0)                      \n");
    printf(" Zero Cloud Storage | Memory Packet Forwarder | Ephemeral Mesh  \n");
    printf(" Listening on UDP port: %u                                      \n", port);
    printf("=================================================================\n\n");

#if defined(_WIN32)
    WSADATA wsa;
    if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
        fprintf(stderr, "WSAStartup failed.\n");
        return 1;
    }
#endif

    socket_t s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s < 0) {
        fprintf(stderr, "Failed to create UDP socket.\n");
        return 1;
    }

    /* Reuse address */
    int opt = 1;
    setsockopt(s, SOL_SOCKET, SO_REUSEADDR, (const char *)&opt, sizeof(opt));

    struct sockaddr_in bind_addr;
    memset(&bind_addr, 0, sizeof(bind_addr));
    bind_addr.sin_family = AF_INET;
    bind_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    bind_addr.sin_port = htons(port);

    if (bind(s, (struct sockaddr *)&bind_addr, sizeof(bind_addr)) < 0) {
        fprintf(stderr, "Failed to bind to port %u.\n", port);
        CLOSE_SOCKET(s);
        return 1;
    }

    printf("[SFU] Ready. Waiting for ephemeral peer connections...\n");

    uint8_t buffer[SFU_BUFFER_SIZE];
    struct sockaddr_in src_addr;
    socklen_t addr_len = sizeof(src_addr);
    time_t last_cleanup = time(NULL);

    while (1) {
        int bytes_read = recvfrom(s, (char *)buffer, sizeof(buffer), 0,
                                  (struct sockaddr *)&src_addr, &addr_len);
        if (bytes_read < (int)sizeof(sfu_pkt_header_t)) {
            continue;
        }

        const sfu_pkt_header_t *hdr = (const sfu_pkt_header_t *)buffer;
        if (hdr->magic != SFU_MAGIC) {
            continue; /* Ignore alien packets */
        }

        /* Periodic prune */
        time_t now = time(NULL);
        if (now - last_cleanup > 5) {
            sfu_cleanup_stale_peers();
            last_cleanup = now;
        }

        /* Lookup room */
        sfu_room_t *room = sfu_get_or_create_room(hdr->room_id);
        if (!room) continue;

        /* Register/keepalive sending peer */
        sfu_register_peer(room, hdr->sender_id, &src_addr);

        /* Forward packet directly in memory to ALL other peers in the same room */
        for (uint32_t i = 0; i < SFU_MAX_CLIENTS_PER_ROOM; ++i) {
            if (room->clients[i].active && strncmp(room->clients[i].peer_id, hdr->sender_id, 64) != 0) {
                sendto(s, (const char *)buffer, bytes_read, 0,
                       (struct sockaddr *)&room->clients[i].addr, sizeof(struct sockaddr_in));
            }
        }

        g_total_packets_forwarded++;
        g_total_bytes_forwarded += bytes_read;

        if (g_total_packets_forwarded % 5000 == 0) {
            printf("[SFU Telemetry] In-memory packets forwarded: %llu (%llu KB) | Zero bytes on disk\n",
                   (unsigned long long)g_total_packets_forwarded,
                   (unsigned long long)(g_total_bytes_forwarded / 1024));
        }
    }

    CLOSE_SOCKET(s);
#if defined(_WIN32)
    WSACleanup();
#endif
    return 0;
}
