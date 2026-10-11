/**
 * Gafana Network Implementation - Real Non-Blocking UDP Socket Engine
 */

#include "gafana_net.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static bool s_wsa_initialized = false;

uint32_t gafana_net_calc_checksum(const void *data, size_t len) {
    const uint8_t *bytes = (const uint8_t *)data;
    uint32_t sum1 = 0xffff, sum2 = 0xffff;

    while (len) {
        size_t tlen = len > 359 ? 359 : len;
        len -= tlen;
        do {
            sum1 += *bytes++;
            sum2 += sum1;
        } while (--tlen);
        sum1 = (sum1 & 0xffff) + (sum1 >> 16);
        sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    }
    sum1 = (sum1 & 0xffff) + (sum1 >> 16);
    sum2 = (sum2 & 0xffff) + (sum2 >> 16);
    return (sum2 << 16) | sum1;
}

gafana_error_t gafana_net_global_init(void) {
#if defined(_WIN32)
    if (!s_wsa_initialized) {
        WSADATA wsa;
        if (WSAStartup(MAKEWORD(2, 2), &wsa) != 0) {
            return GAFANA_ERR_SOCKET;
        }
        s_wsa_initialized = true;
    }
#endif
    return GAFANA_OK;
}

void gafana_net_global_cleanup(void) {
#if defined(_WIN32)
    if (s_wsa_initialized) {
        WSACleanup();
        s_wsa_initialized = false;
    }
#endif
}

gafana_error_t gafana_net_init(gafana_net_transport_t *net, const char *relay_host, uint16_t relay_port) {
    if (!net) return GAFANA_ERR_INVALID_PARAM;
    memset(net, 0, sizeof(gafana_net_transport_t));
    net->sock = GAFANA_INVALID_SOCKET;

    gafana_error_t err = gafana_net_global_init();
    if (err != GAFANA_OK) return err;

    gafana_socket_t s = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
    if (s == GAFANA_INVALID_SOCKET) {
        return GAFANA_ERR_SOCKET;
    }

    /* Set socket non-blocking */
#if defined(_WIN32)
    u_long non_blocking = 1;
    if (ioctlsocket(s, FIONBIO, &non_blocking) != 0) {
        gafana_socket_close(s);
        return GAFANA_ERR_SOCKET;
    }
#else
    int flags = fcntl(s, F_GETFL, 0);
    if (flags < 0 || fcntl(s, F_SETFL, flags | O_NONBLOCK) < 0) {
        gafana_socket_close(s);
        return GAFANA_ERR_SOCKET;
    }
#endif

    /* Enable broadcast */
    int broadcast_opt = 1;
    setsockopt(s, SOL_SOCKET, SO_BROADCAST, (const char *)&broadcast_opt, sizeof(broadcast_opt));

    /* Bind to any available local ephemeral port */
    struct sockaddr_in local_addr;
    memset(&local_addr, 0, sizeof(local_addr));
    local_addr.sin_family = AF_INET;
    local_addr.sin_addr.s_addr = htonl(INADDR_ANY);
    local_addr.sin_port = 0; /* Auto assign */

    if (bind(s, (struct sockaddr *)&local_addr, sizeof(local_addr)) < 0) {
        gafana_socket_close(s);
        return GAFANA_ERR_SOCKET;
    }

    /* Query assigned port */
    socklen_t addr_len = sizeof(local_addr);
    if (getsockname(s, (struct sockaddr *)&local_addr, &addr_len) == 0) {
        net->local_port = ntohs(local_addr.sin_port);
    }

    net->sock = s;
    net->is_bound = true;
    net->next_seq = 1;

    /* Setup relay destination if provided */
    if (relay_host && strlen(relay_host) > 0 && strcmp(relay_host, "local") != 0) {
        memset(&net->relay_addr, 0, sizeof(net->relay_addr));
        net->relay_addr.sin_family = AF_INET;
        net->relay_addr.sin_port = htons(relay_port > 0 ? relay_port : GAFANA_DEFAULT_PORT);

        struct hostent *he = gethostbyname(relay_host);
        if (he && he->h_addr_list && he->h_addr_list[0]) {
            memcpy(&net->relay_addr.sin_addr, he->h_addr_list[0], he->h_length);
            net->is_connected_to_relay = true;
        } else {
            /* Fallback to loopback if resolution fails */
            net->relay_addr.sin_addr.s_addr = inet_addr("127.0.0.1");
            net->is_connected_to_relay = true;
        }
    } else {
        /* Default local loopback destination */
        memset(&net->relay_addr, 0, sizeof(net->relay_addr));
        net->relay_addr.sin_family = AF_INET;
        net->relay_addr.sin_port = htons(relay_port > 0 ? relay_port : GAFANA_DEFAULT_PORT);
        net->relay_addr.sin_addr.s_addr = inet_addr("127.0.0.1");
        net->is_connected_to_relay = true;
    }

    return GAFANA_OK;
}

void gafana_net_close(gafana_net_transport_t *net) {
    if (!net) return;
    if (net->sock != GAFANA_INVALID_SOCKET) {
        gafana_socket_close(net->sock);
        net->sock = GAFANA_INVALID_SOCKET;
    }
    net->is_bound = false;
    net->is_connected_to_relay = false;
}

gafana_error_t gafana_net_send(
    gafana_net_transport_t *net,
    const char *room_id,
    const char *sender_id,
    uint8_t opcode,
    const void *payload,
    uint16_t payload_len
) {
    if (!net || net->sock == GAFANA_INVALID_SOCKET || !room_id || !sender_id) {
        return GAFANA_ERR_INVALID_PARAM;
    }

    if (payload_len > (GAFANA_NET_MTU - sizeof(gafana_net_packet_header_t))) {
        /* Chunking / fragmentation for large video frames */
        uint16_t max_chunk_payload = GAFANA_NET_MTU - (uint16_t)sizeof(gafana_net_packet_header_t);
        uint16_t total_chunks = (payload_len + max_chunk_payload - 1) / max_chunk_payload;
        const uint8_t *pbytes = (const uint8_t *)payload;

        for (uint16_t c = 0; c < total_chunks; ++c) {
            uint16_t this_chunk_len = (c == total_chunks - 1) ? (payload_len - (c * max_chunk_payload)) : max_chunk_payload;
            uint8_t packet_buffer[GAFANA_MAX_PACKET_SIZE];
            gafana_net_packet_header_t *hdr = (gafana_net_packet_header_t *)packet_buffer;

            hdr->magic = 0x47414641; /* "GAFA" */
            hdr->version = 1;
            hdr->opcode = opcode;
            hdr->flags = 0;
            hdr->seq = net->next_seq++;
            hdr->timestamp_ms = (uint32_t)GetTickCount();
            strncpy(hdr->room_id, room_id, GAFANA_MAX_ID_LEN - 1);
            strncpy(hdr->sender_id, sender_id, GAFANA_MAX_ID_LEN - 1);
            hdr->payload_len = this_chunk_len;
            hdr->chunk_idx = c;
            hdr->total_chunks = total_chunks;

            memcpy(packet_buffer + sizeof(gafana_net_packet_header_t), pbytes + (c * max_chunk_payload), this_chunk_len);
            hdr->checksum = gafana_net_calc_checksum(packet_buffer + sizeof(gafana_net_packet_header_t), this_chunk_len);

            size_t total_wire_len = sizeof(gafana_net_packet_header_t) + this_chunk_len;
            sendto(net->sock, (const char *)packet_buffer, (int)total_wire_len, 0,
                   (struct sockaddr *)&net->relay_addr, sizeof(net->relay_addr));
        }
        return GAFANA_OK;
    }

    uint8_t packet_buffer[GAFANA_MAX_PACKET_SIZE];
    gafana_net_packet_header_t *hdr = (gafana_net_packet_header_t *)packet_buffer;

    hdr->magic = 0x47414641;
    hdr->version = 1;
    hdr->opcode = opcode;
    hdr->flags = 0;
    hdr->seq = net->next_seq++;
#if defined(_WIN32)
    hdr->timestamp_ms = (uint32_t)GetTickCount();
#else
    hdr->timestamp_ms = 0;
#endif
    strncpy(hdr->room_id, room_id, GAFANA_MAX_ID_LEN - 1);
    strncpy(hdr->sender_id, sender_id, GAFANA_MAX_ID_LEN - 1);
    hdr->payload_len = payload_len;
    hdr->chunk_idx = 0;
    hdr->total_chunks = 1;

    if (payload && payload_len > 0) {
        memcpy(packet_buffer + sizeof(gafana_net_packet_header_t), payload, payload_len);
        hdr->checksum = gafana_net_calc_checksum(payload, payload_len);
    } else {
        hdr->checksum = 0;
    }

    size_t total_wire_len = sizeof(gafana_net_packet_header_t) + payload_len;
    int sent = sendto(net->sock, (const char *)packet_buffer, (int)total_wire_len, 0,
                      (struct sockaddr *)&net->relay_addr, sizeof(net->relay_addr));

    return (sent > 0) ? GAFANA_OK : GAFANA_ERR_NETWORK;
}

int gafana_net_recv(
    gafana_net_transport_t *net,
    uint8_t *out_buffer,
    size_t max_len,
    gafana_net_endpoint_t *out_src
) {
    if (!net || net->sock == GAFANA_INVALID_SOCKET || !out_buffer) return -1;

    struct sockaddr_storage src_addr;
    socklen_t src_len = sizeof(src_addr);

    int received = recvfrom(net->sock, (char *)out_buffer, (int)max_len, 0,
                            (struct sockaddr *)&src_addr, &src_len);

    if (received <= 0) {
        return -1; /* No data (non-blocking) or error */
    }

    if (out_src) {
        memcpy(&out_src->addr, &src_addr, src_len);
        out_src->addr_len = src_len;
        if (src_addr.ss_family == AF_INET) {
            struct sockaddr_in *sin = (struct sockaddr_in *)&src_addr;
            inet_ntop(AF_INET, &sin->sin_addr, out_src->ip_str, sizeof(out_src->ip_str));
            out_src->port = ntohs(sin->sin_port);
        }
    }

    /* Basic sanity check on packet header */
    if ((size_t)received < sizeof(gafana_net_packet_header_t)) {
        return -1;
    }

    const gafana_net_packet_header_t *hdr = (const gafana_net_packet_header_t *)out_buffer;
    if (hdr->magic != 0x47414641) {
        return -1; /* Invalid magic */
    }

    /* Verify checksum on payload */
    if (hdr->payload_len > 0 && ((size_t)received >= sizeof(gafana_net_packet_header_t) + hdr->payload_len)) {
        uint32_t calc = gafana_net_calc_checksum(out_buffer + sizeof(gafana_net_packet_header_t), hdr->payload_len);
        if (calc != hdr->checksum) {
            return -1; /* Checksum mismatch: discard corrupted packet */
        }
    }

    return received;
}
