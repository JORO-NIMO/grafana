/**
 * Gafana Network & Transport Layer (UDP / Non-Blocking Datagram Relay)
 *
 * Provides real socket I/O, packet fragmentation, and checksum verification.
 */

#ifndef GAFANA_NET_H
#define GAFANA_NET_H

#include "../include/gafana.h"

#if defined(_WIN32)
  #include "gafana_winsock.h"
  typedef SOCKET gafana_socket_t;
  #define GAFANA_INVALID_SOCKET INVALID_SOCKET
  #define gafana_socket_close(s) closesocket(s)
#else
  #include <sys/types.h>
  #include <sys/socket.h>
  #include <netinet/in.h>
  #include <arpa/inet.h>
  #include <netdb.h>
  #include <unistd.h>
  #include <fcntl.h>
  typedef int gafana_socket_t;
  #define GAFANA_INVALID_SOCKET (-1)
  #define gafana_socket_close(s) close(s)
#endif

#define GAFANA_NET_MTU 1400
#define GAFANA_MAX_PACKET_SIZE 1472

#pragma pack(push, 1)
typedef struct gafana_net_packet_header_s {
    uint32_t magic;           /* GAFANA_MAGIC (0x47414641) */
    uint8_t  version;         /* 1 */
    uint8_t  opcode;          /* GAFANA_OP_* */
    uint16_t flags;
    uint32_t seq;             /* Monotonically increasing packet sequence */
    uint32_t timestamp_ms;
    char     room_id[GAFANA_MAX_ID_LEN];
    char     sender_id[GAFANA_MAX_ID_LEN];
    uint16_t payload_len;
    uint16_t chunk_idx;       /* For MTU fragmentation */
    uint16_t total_chunks;
    uint32_t checksum;        /* Fast Fletcher32 / CRC */
} gafana_net_packet_header_t;
#pragma pack(pop)

typedef struct gafana_net_endpoint_s {
    struct sockaddr_storage addr;
    socklen_t addr_len;
    char ip_str[64];
    uint16_t port;
} gafana_net_endpoint_t;

typedef struct gafana_net_transport_s {
    gafana_socket_t sock;
    bool is_bound;
    bool is_connected_to_relay;
    struct sockaddr_in relay_addr;
    uint32_t local_port;
    uint32_t next_seq;
} gafana_net_transport_t;

/* Networking API */
gafana_error_t gafana_net_global_init(void);
void gafana_net_global_cleanup(void);

gafana_error_t gafana_net_init(gafana_net_transport_t *net, const char *relay_host, uint16_t relay_port);
void gafana_net_close(gafana_net_transport_t *net);

gafana_error_t gafana_net_send(
    gafana_net_transport_t *net,
    const char *room_id,
    const char *sender_id,
    uint8_t opcode,
    const void *payload,
    uint16_t payload_len
);

int gafana_net_recv(
    gafana_net_transport_t *net,
    uint8_t *out_buffer,
    size_t max_len,
    gafana_net_endpoint_t *out_src
);

uint32_t gafana_net_calc_checksum(const void *data, size_t len);

#endif /* GAFANA_NET_H */
