#ifndef GAFANA_SMTP_H
#define GAFANA_SMTP_H

#include "../include/gafana.h"
#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* gafana_smtp_config_t is defined in ../include/gafana.h */

/**
 * Configure SMTP service in-memory in the executable.
 * All credentials are held purely in volatile RAM (Zero Cloud / Zero Disk retention).
 */
gafana_error_t gafana_smtp_setup(const gafana_smtp_config_t *config);

/**
 * Retrieve the current active SMTP configuration (passwords masked).
 */
gafana_error_t gafana_smtp_get_config(gafana_smtp_config_t *out_config);

/**
 * Send a secure, tamper-evident meeting invitation to an attendee via SMTP.
 */
gafana_error_t gafana_smtp_send_invite(
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

#endif /* GAFANA_SMTP_H */
