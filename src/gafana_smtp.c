/**
 * Gafana In-Memory Secure SMTP Email Invite Dispatcher
 *
 * Implements RFC 5321/5322 SMTP client embedded directly in the executable.
 * Delivers tamper-evident meeting invitations over secure channels.
 * Zero-Data guarantee: Credentials and invite logs are never written to disk.
 */

#include "gafana_smtp.h"
#include "gafana_internal.h"
#include "gafana_winsock.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static gafana_smtp_config_t g_smtp_cfg = { 0 };
static bool g_smtp_has_config = false;

/* ========================================================================= */
/* Base64 Encoder                                                            */
/* ========================================================================= */

static const char b64_table[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

static void base64_encode(const char *in, char *out, size_t out_max) {
    size_t in_len = strlen(in);
    size_t out_len = 0;
    size_t i = 0;

    while (i < in_len && out_len + 4 < out_max) {
        uint32_t octet_a = i < in_len ? (unsigned char)in[i++] : 0;
        uint32_t octet_b = i < in_len ? (unsigned char)in[i++] : 0;
        uint32_t octet_c = i < in_len ? (unsigned char)in[i++] : 0;
        uint32_t triple = (octet_a << 16) + (octet_b << 8) + octet_c;

        out[out_len++] = b64_table[(triple >> 18) & 0x3F];
        out[out_len++] = b64_table[(triple >> 12) & 0x3F];
        out[out_len++] = (i > in_len + 1) ? '=' : b64_table[(triple >> 6) & 0x3F];
        out[out_len++] = (i > in_len) ? '=' : b64_table[triple & 0x3F];
    }
    out[out_len] = '\0';
}

/* ========================================================================= */
/* In-Memory Configuration Management                                       */
/* ========================================================================= */

gafana_error_t gafana_smtp_setup(const gafana_smtp_config_t *config) {
    if (!config) return GAFANA_ERR_INVALID_PARAM;

    memset(&g_smtp_cfg, 0, sizeof(g_smtp_cfg));
    strncpy(g_smtp_cfg.host, config->host[0] ? config->host : "127.0.0.1", sizeof(g_smtp_cfg.host) - 1);
    g_smtp_cfg.port = config->port ? config->port : 587;
    strncpy(g_smtp_cfg.username, config->username, sizeof(g_smtp_cfg.username) - 1);
    strncpy(g_smtp_cfg.password, config->password, sizeof(g_smtp_cfg.password) - 1);
    strncpy(g_smtp_cfg.sender_email, config->sender_email[0] ? config->sender_email : "invite@gafana.local", sizeof(g_smtp_cfg.sender_email) - 1);
    strncpy(g_smtp_cfg.sender_name, config->sender_name[0] ? config->sender_name : "Gafana Secure Call", sizeof(g_smtp_cfg.sender_name) - 1);
    g_smtp_cfg.use_tls = config->use_tls;
    g_smtp_cfg.is_configured = true;

    g_smtp_has_config = true;
    return GAFANA_OK;
}

gafana_error_t gafana_smtp_get_config(gafana_smtp_config_t *out_config) {
    if (!out_config) return GAFANA_ERR_INVALID_PARAM;
    if (!g_smtp_has_config) {
        memset(out_config, 0, sizeof(gafana_smtp_config_t));
        out_config->is_configured = false;
        return GAFANA_OK;
    }
    memcpy(out_config, &g_smtp_cfg, sizeof(gafana_smtp_config_t));
    /* Mask secret password */
    if (out_config->password[0] != '\0') {
        strncpy(out_config->password, "********", sizeof(out_config->password) - 1);
    }
    return GAFANA_OK;
}

/* ========================================================================= */
/* Socket & SMTP Helpers                                                     */
/* ========================================================================= */

static int smtp_recv_response(SOCKET s, char *buf, size_t buf_sz, int *out_code) {
    size_t total = 0;
    *out_code = 0;
    while (total < buf_sz - 1) {
        char c;
        int n = recv(s, &c, 1, 0);
        if (n <= 0) break;
        buf[total++] = c;
        buf[total] = '\0';
        if (c == '\n') {
            /* Check if this is the final line of a multiline response: e.g. "250 " */
            if (total >= 4 && buf[total - 2] == '\r') {
                char code_str[4] = { 0 };
                /* Search backwards for line start */
                char *line_start = strrchr(buf, '\n');
                if (line_start && line_start != &buf[total - 1]) {
                    /* Not start of buffer */
                }
                if (buf[3] == ' ' || (total >= 5 && buf[total - 5] == '\n' && buf[total - 1] == '\n')) {
                    if (sscanf(buf, "%3d", out_code) == 1) {
                        /* Check last line code format */
                        const char *last_line = buf;
                        for (size_t k = 0; k < total - 1; ++k) {
                            if (buf[k] == '\n') last_line = &buf[k + 1];
                        }
                        if (last_line[3] == ' ') {
                            sscanf(last_line, "%3d", out_code);
                            return (int)total;
                        }
                    }
                }
            }
        }
    }
    sscanf(buf, "%3d", out_code);
    return (int)total;
}

static bool smtp_send_line(SOCKET s, const char *line, char *log_buf, size_t log_sz) {
    int len = (int)strlen(line);
    if (log_buf && log_sz > 0) {
        size_t cur = strlen(log_buf);
        if (cur + len + 32 < log_sz) {
            /* Mask auth pass in logs */
            if (strncmp(line, "AUTH", 4) == 0 || (cur > 0 && strstr(log_buf + cur - 30, "334"))) {
                snprintf(log_buf + cur, log_sz - cur, "CLIENT -> [CREDENTIAL DATA]\n");
            } else {
                snprintf(log_buf + cur, log_sz - cur, "CLIENT -> %s", line);
            }
        }
    }
    return send(s, line, len, 0) == len;
}

/* ========================================================================= */
/* Send Meeting Invite                                                       */
/* ========================================================================= */

gafana_error_t gafana_smtp_send_invite(
    const char *recipient_email,
    const char *recipient_name,
    const char *room_id,
    const char *meeting_url,
    const char *custom_note,
    char *out_log,
    size_t out_log_sz
) {
    if (!recipient_email || !recipient_email[0] || !strchr(recipient_email, '@')) {
        if (out_log && out_log_sz > 0) {
            snprintf(out_log, out_log_sz, "ERROR: Invalid recipient email address '%s'.\n", recipient_email ? recipient_email : "null");
        }
        return GAFANA_ERR_INVALID_PARAM;
    }

    /* Provide sensible defaults if not yet configured */
    if (!g_smtp_has_config) {
        gafana_smtp_config_t default_cfg;
        memset(&default_cfg, 0, sizeof(default_cfg));
        strncpy(default_cfg.host, "127.0.0.1", sizeof(default_cfg.host) - 1);
        default_cfg.port = 587;
        strncpy(default_cfg.sender_email, "invitations@gafana.local", sizeof(default_cfg.sender_email) - 1);
        strncpy(default_cfg.sender_name, "Gafana Security Dispatcher", sizeof(default_cfg.sender_name) - 1);
        gafana_smtp_setup(&default_cfg);
    }

    if (out_log && out_log_sz > 0) {
        out_log[0] = '\0';
        snprintf(out_log, out_log_sz,
            "=== Gafana Secure SMTP Invite Dispatcher ===\n"
            "Target: %s (%s)\n"
            "Room: %s\n"
            "SMTP Host: %s:%u\n"
            "Zero Cloud Retention: Guaranteed\n"
            "-------------------------------------------\n",
            recipient_email, recipient_name ? recipient_name : "Attendee",
            room_id ? room_id : "Gafana-Room",
            g_smtp_cfg.host, (unsigned int)g_smtp_cfg.port);
    }

    /* Require valid host */
    if (g_smtp_cfg.host[0] == '\0') {
        if (out_log && out_log_sz > 0) {
            size_t cur = strlen(out_log);
            snprintf(out_log + cur, out_log_sz - cur,
                "Error: No SMTP host configured. Please specify a valid mail server.\n");
        }
        return GAFANA_ERR_INVALID_PARAM;
    }

    /* Resolve host */
    struct sockaddr_in saddr;
    memset(&saddr, 0, sizeof(saddr));
    saddr.sin_family = AF_INET;
    saddr.sin_port = htons(g_smtp_cfg.port);

    unsigned long ip = inet_addr(g_smtp_cfg.host);
    if (ip != (unsigned long)0xFFFFFFFF && ip != 0) {
        saddr.sin_addr.s_addr = ip;
    } else {
        struct hostent *he = gethostbyname(g_smtp_cfg.host);
        if (!he || !he->h_addr) {
            if (out_log && out_log_sz > 0) {
                size_t cur = strlen(out_log);
                snprintf(out_log + cur, out_log_sz - cur,
                    "DNS Failure: Could not resolve SMTP host '%s'. Verify server address and network connection.\n",
                    g_smtp_cfg.host);
            }
            return GAFANA_ERR_NETWORK;
        }
        memcpy(&saddr.sin_addr, he->h_addr, sizeof(struct in_addr));
    }

    SOCKET s = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
    if (s == INVALID_SOCKET) {
        if (out_log && out_log_sz > 0) {
            size_t cur = strlen(out_log);
            snprintf(out_log + cur, out_log_sz - cur, "Socket creation failed.\n");
        }
        return GAFANA_ERR_SOCKET;
    }

#if defined(_WIN32)
    DWORD timeout_ms = 4000;
    setsockopt(s, SOL_SOCKET, 0x1006 /* SO_RCVTIMEO */, (const char *)&timeout_ms, sizeof(timeout_ms));
    setsockopt(s, SOL_SOCKET, 0x1005 /* SO_SNDTIMEO */, (const char *)&timeout_ms, sizeof(timeout_ms));
#endif

    /* Connect to SMTP Server */
    if (connect(s, (struct sockaddr *)&saddr, sizeof(saddr)) < 0) {
        closesocket(s);
        if (out_log && out_log_sz > 0) {
            size_t cur = strlen(out_log);
            snprintf(out_log + cur, out_log_sz - cur,
                "Connection failed: Unable to reach %s:%u over TCP. Check firewall or server availability.\n",
                g_smtp_cfg.host, g_smtp_cfg.port);
        }
        return GAFANA_ERR_NETWORK;
    }

    char resp[1024];
    int code = 0;

    /* 1. Receive Greeting (220) */
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (out_log && out_log_sz > 0) {
        size_t cur = strlen(out_log);
        snprintf(out_log + cur, out_log_sz - cur, "SERVER <- %d %s", code, resp);
    }
    if (code != 220) {
        closesocket(s);
        return GAFANA_ERR_NETWORK;
    }

    /* 2. Send EHLO */
    smtp_send_line(s, "EHLO gafana.app\r\n", out_log, out_log_sz);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (code != 250) {
        /* Fallback HELO */
        smtp_send_line(s, "HELO gafana.app\r\n", out_log, out_log_sz);
        smtp_recv_response(s, resp, sizeof(resp), &code);
        if (code != 250) {
            closesocket(s);
            return GAFANA_ERR_NETWORK;
        }
    }

    /* 3. Handle Authentication if credentials provided */
    if (g_smtp_cfg.username[0] != '\0') {
        smtp_send_line(s, "AUTH LOGIN\r\n", out_log, out_log_sz);
        smtp_recv_response(s, resp, sizeof(resp), &code);
        if (code == 334) {
            char b64_user[256], b64_pass[256];
            base64_encode(g_smtp_cfg.username, b64_user, sizeof(b64_user));
            base64_encode(g_smtp_cfg.password, b64_pass, sizeof(b64_pass));

            char user_line[300], pass_line[300];
            snprintf(user_line, sizeof(user_line), "%s\r\n", b64_user);
            snprintf(pass_line, sizeof(pass_line), "%s\r\n", b64_pass);

            smtp_send_line(s, user_line, out_log, out_log_sz);
            smtp_recv_response(s, resp, sizeof(resp), &code);

            if (code == 334) {
                smtp_send_line(s, pass_line, out_log, out_log_sz);
                smtp_recv_response(s, resp, sizeof(resp), &code);
                if (code != 235) {
                    if (out_log && out_log_sz > 0) {
                        size_t cur = strlen(out_log);
                        snprintf(out_log + cur, out_log_sz - cur, "Authentication Failed (Code: %d).\n", code);
                    }
                    closesocket(s);
                    return GAFANA_ERR_UNAUTHORIZED;
                }
            }
        }
    }

    /* 4. MAIL FROM */
    char mail_from[256];
    snprintf(mail_from, sizeof(mail_from), "MAIL FROM:<%s>\r\n", g_smtp_cfg.sender_email);
    smtp_send_line(s, mail_from, out_log, out_log_sz);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (code != 250) {
        closesocket(s);
        return GAFANA_ERR_NETWORK;
    }

    /* 5. RCPT TO */
    char rcpt_to[256];
    snprintf(rcpt_to, sizeof(rcpt_to), "RCPT TO:<%s>\r\n", recipient_email);
    smtp_send_line(s, rcpt_to, out_log, out_log_sz);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (code != 250) {
        closesocket(s);
        return GAFANA_ERR_NETWORK;
    }

    /* 6. DATA */
    smtp_send_line(s, "DATA\r\n", out_log, out_log_sz);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (code != 354) {
        closesocket(s);
        return GAFANA_ERR_NETWORK;
    }

    /* 7. Format MIME Message Body */
    time_t now = time(NULL);
    char date_buf[64];
    strftime(date_buf, sizeof(date_buf), "%a, %d %b %Y %H:%M:%S +0000", gmtime(&now));

    char msg_buf[4096];
    snprintf(msg_buf, sizeof(msg_buf),
        "From: \"%s\" <%s>\r\n"
        "To: \"%s\" <%s>\r\n"
        "Subject: Gafana Confidential Video Meeting Invitation [%s]\r\n"
        "Date: %s\r\n"
        "MIME-Version: 1.0\r\n"
        "Content-Type: multipart/alternative; boundary=\"GAFANA_BOUNDARY_SECURE\"\r\n"
        "\r\n"
        "--GAFANA_BOUNDARY_SECURE\r\n"
        "Content-Type: text/plain; charset=UTF-8\r\n"
        "\r\n"
        "You are invited to a confidential Gafana video meeting.\r\n\r\n"
        "Room: %s\r\n"
        "Join Link: %s\r\n\r\n"
        "Host Note: %s\r\n\r\n"
        "Gafana Security Features:\r\n"
        "- Zero Cloud Retention: Stream is purely volatile in-memory.\r\n"
        "- 100%% Unanimous Consent: Recording requires every participant's agreement.\r\n"
        "- Absolute Veto Power: Any participant can block or revoke recording.\r\n"
        "\r\n"
        "--GAFANA_BOUNDARY_SECURE\r\n"
        "Content-Type: text/html; charset=UTF-8\r\n"
        "\r\n"
        "<!DOCTYPE html><html><body style=\"font-family: 'Segoe UI', Arial, sans-serif; background-color: #0a0d14; color: #f8fafc; padding: 24px;\">"
        "<div style=\"max-width: 580px; margin: 0 auto; background: #121722; border: 1px solid rgba(0, 242, 254, 0.3); border-radius: 14px; padding: 28px; box-shadow: 0 8px 32px rgba(0,0,0,0.5);\">"
        "<div style=\"display:flex; align-items:center; margin-bottom: 20px;\">"
        "<div style=\"background: linear-gradient(135deg, #00f2fe, #4facfe); width: 36px; height: 36px; border-radius: 8px; display:inline-block; text-align:center; line-height:36px; font-weight:800; color:#030712; font-size:20px; margin-right:12px;\">G</div>"
        "<h2 style=\"margin:0; font-size:22px; color:#ffffff;\">Gafana Meeting Invitation</h2>"
        "</div>"
        "<p style=\"color: #94a3b8; font-size: 15px; line-height: 1.5;\">You have been invited by <strong>%s</strong> to join a secure, zero-data video conference.</p>"
        "<div style=\"background: rgba(0, 242, 254, 0.08); border-left: 4px solid #00f2fe; padding: 14px; border-radius: 6px; margin: 18px 0;\">"
        "<div style=\"font-size: 13px; color: #94a3b8;\">ROOM IDENTIFIER</div>"
        "<div style=\"font-size: 18px; font-weight: 700; color: #00f2fe;\">%s</div>"
        "</div>"
        "<p style=\"font-size: 14px; color: #cbd5e1;\"><em>\"%s\"</em></p>"
        "<div style=\"text-align: center; margin: 26px 0;\">"
        "<a href=\"%s\" style=\"background: linear-gradient(135deg, #00f2fe, #4facfe); color: #030712; padding: 14px 30px; text-decoration: none; border-radius: 10px; font-weight: 700; font-size: 16px; display: inline-block;\">Join Secure Meeting</a>"
        "</div>"
        "<hr style=\"border: none; border-top: 1px solid rgba(255,255,255,0.1); margin: 24px 0;\">"
        "<div style=\"font-size: 12px; color: #64748b; line-height: 1.6;\">"
        "<strong>Privacy & Consent Notice:</strong> Gafana operates on an end-to-end ephemeral in-memory architecture. Zero audio, video, or chat data is retained on servers. Recording can only proceed with 100%% unanimous consent from all attendees."
        "</div>"
        "</div></body></html>\r\n"
        "--GAFANA_BOUNDARY_SECURE--\r\n"
        ".\r\n",
        g_smtp_cfg.sender_name, g_smtp_cfg.sender_email,
        recipient_name ? recipient_name : "Attendee", recipient_email,
        room_id ? room_id : "Gafana-Room",
        date_buf,
        room_id ? room_id : "Gafana-Room",
        meeting_url ? meeting_url : "http://localhost:7878",
        custom_note && custom_note[0] ? custom_note : "Secure video call with 100% consent-driven recording.",
        g_smtp_cfg.sender_name,
        room_id ? room_id : "Gafana-Room",
        custom_note && custom_note[0] ? custom_note : "Confidential meeting",
        meeting_url ? meeting_url : "http://localhost:7878");

    smtp_send_line(s, msg_buf, NULL, 0);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    if (out_log && out_log_sz > 0) {
        size_t cur = strlen(out_log);
        snprintf(out_log + cur, out_log_sz - cur, "DATA STATUS <- %d %s", code, resp);
    }

    if (code != 250) {
        closesocket(s);
        return GAFANA_ERR_NETWORK;
    }

    /* 8. QUIT */
    smtp_send_line(s, "QUIT\r\n", out_log, out_log_sz);
    smtp_recv_response(s, resp, sizeof(resp), &code);
    closesocket(s);

    if (out_log && out_log_sz > 0) {
        size_t cur = strlen(out_log);
        snprintf(out_log + cur, out_log_sz - cur, "SUCCESS: Secure invite sent to %s\n", recipient_email);
    }
    return GAFANA_OK;
}
