# Safe Spaces Grafana: Zero-Data Real-Time Conference Engine

**Safe Spaces Grafana** is an all-in-one, privacy-first, zero-data real-time video conferencing engine developed by **Safe Spaces AI Innovation Hub - SMC Limited** (Mbarara, Uganda).

The entire system is packaged into a **single unified appliance (`grafana`)** containing the core media engine, in-memory stateless SFU relay, embedded WebRTC UI server, desktop console, and self-hosted SMTP client.

Built with **zero external crate dependencies** using pure Rust standard library (`std`) for maximum performance, minimal attack surface, and instant compilation.

---

## Production Deployments & Domains

- **Live Conference:** [https://grafana.safespaceshub.com](https://grafana.safespaceshub.com)
- **Dedicated Mail Server (Stalwart SMTP):** `mail.safespaceshub.com:587`
- **Voluntary Cloud Backup Vault:** [https://filefran.safespaceshub.com](https://filefran.safespaceshub.com)
- **Safe Spaces AI Innovation Hub Portal:** [https://safespaceshub.com](https://safespaceshub.com)

---

## Core Architectural Principles

### 1. Local-First Device Storage (Default)
- All user personal info, meeting logs, session transcripts, consensus ballots, and key actions remain strictly inside the user's browser sandbox on their local device.
- Video and audio streams are processed in volatile RAM and transmitted peer-to-peer or via an in-memory stateless relay with zero disk or database retention.
- No third-party analytics, no tracking pixels, and zero behavioral telemetry.

### 2. Host Authentication & Email Confirmation
- **Sign Up:** Registration requires `username`, `email`, and `password`.
- **6-Digit Confirmation Code:** Upon registration, a 6-digit confirmation code with a 30-minute expiration is dispatched to the user's email via our self-hosted **Stalwart Mail Server** (`mail.safespaceshub.com`).
- **Gated Hosting Privileges:** Only confirmed and verified host accounts can dispatch meeting invitations or access host administrative controls. Unauthenticated or unverified users are strictly prevented from sending invitations (HTTP 401/403).

### 3. Voluntary Cloud Backup (`filefran.safespaceshub.com`)
- Users only sync and back up their data to our cloud if they explicitly choose to opt in via the **Device Storage & Backup** settings.
- When enabled, encrypted snapshots of meeting configurations and local records are automatically pushed to our dedicated vault at **`https://filefran.safespaceshub.com`**.
- Users retain absolute ownership: they can export their complete local history as JSON, toggle cloud backup off at any time, or execute an instant local storage purge.

### 4. Admin Monitoring Dashboard
- Dedicated administrative overview (`/api/admin/overview`) accessible to authorized administrators (`admin@safespaceshub.com` / `is_admin`).
- Allows monitoring of voluntary shared user data, active accounts, host verification statuses, cloud backup sizes, and real-time active conference rooms.

### 5. Multi-Party Consensus Recording
- Server-side recording is prohibited. Recording is client-local only and requires **100% unanimous agreement** from all active participants.
- Late-joining participants who find a recording already underway proceed seamlessly with what has already been approved and do not need to re-ballot or disrupt the call.
- Any participant can veto a proposed recording or revoke consent mid-call, which immediately halts capture for all parties.

### 6. Zero-Trust Browser Extension Shield & Anti-Capture Guard
- **Extension Shield:** Real-time DOM `MutationObserver` unmounts and purges unauthorized third-party extensions, injected scripts, and trackers.
- **Anti-Capture Protection:** Neutralizes unauthorized screenshot commands (`PrintScreen`, `Win+Shift+S`), purges the clipboard, and displays an opaque blackout privacy overlay if external screen grabbers gain focus.
- **Background Privacy Mute:** Automatically halts microphone and camera streams when the conference tab is hidden or minimized.

---

## Regulatory Compliance & Privacy Laws

Safe Spaces Grafana adheres to national, continental, and international privacy standards:
1. **Uganda Data Protection and Privacy Act 2019:** Full compliance with principles of data minimization, lawful processing, and storage limitation.
2. **Kenya Data Protection Act 2019:** Strict adherence to data subject consent, cross-border transfer safeguards, and confidentiality.
3. **African Union Cyber Security & Personal Data Protection Convention (Malabo Convention):** Alignment with continental digital sovereignty, institutional protection, and human rights in cyberspace.
4. **EU General Data Protection Regulation (GDPR):** Respect for data subject rights (Articles 15-20: access, rectification, erasure, and portability).

---

## Cookie & Storage Policy

- **Zero Tracking / Advertising Cookies:** Safe Spaces Grafana uses zero third-party cookies, zero advertising networks, and zero tracking pixels.
- **Strictly Functional Local Storage:** The browser stores only strictly necessary functional tokens:
  - `grafana_token`: Authenticated host session token.
  - `grafana_profile`: Preferred display name and custom avatar choice.
  - `grafana_cloud_sync`: Voluntary consent flag for backups to `filefran.safespaceshub.com`.
  - `grafana_settings`: Audio, video, and auto-admit configurations.

---

## Quick Start (Cargo)

Run the single unified appliance with standard Cargo:

```powershell
cargo run
```

When started, `grafana` automatically:
1. Launches the in-memory Stateless SFU Relay on UDP port `8765`.
2. Launches the embedded HTTP Web Server on TCP port `7878` serving the clean UI.
3. Auto-opens the interface in your browser (`http://localhost:7878`).
4. Starts the interactive desktop console in your terminal.

Default administrator credentials:
- **Username / Email:** `admin@safespaceshub.com`
- **Default Password:** `SafeSpaces2026!` (configurable via `ADMIN_PASSWORD` environment variable)

---

## Interactive Console Commands

While `grafana` is running, the terminal console accepts:

| Command | Action |
| :--- | :--- |
| `peers` | List active room participants |
| `record [a\|v]` | Request unanimous consent to record (`a`=audio-only, `v`=full AV) |
| `startrec` | Start local disk capture (strictly blocked unless 100% approved) |
| `stoprec` | Stop local recording and finalize file header |
| `accept` | Vote YES on a pending recording consent ballot |
| `deny` | Vote NO (VETO) on a pending recording consent ballot |
| `revoke` | Revoke consent mid-call (emergency forced halt of recording) |
| `screen` | Toggle native OS screen sharing |
| `present` | Start synchronized presentation mode (10 slides) |
| `next` | Advance presentation slide |
| `stats` | Display in-memory packet forwarding throughput |
| `open` | Re-open the WebRTC interface in the browser |
| `quit` | Cleanly terminate engine, relay, and web server |

---

## REST API Overview

| Endpoint | Method | Auth | Description |
| :--- | :--- | :--- | :--- |
| `/api/auth/register` | `POST` | Public | Register host account; dispatches 6-digit code via Stalwart SMTP |
| `/api/auth/verify` | `POST` | Public | Confirm email with 6-digit code; activates host status |
| `/api/auth/login` | `POST` | Public | Authenticate with username/email & password; returns session token |
| `/api/auth/me` | `GET` | Bearer Token | Return current host profile, verification status, and sync settings |
| `/api/auth/logout` | `POST` | Bearer Token | Invalidate current session token |
| `/api/cloud/sync` | `POST` | Verified Host | Voluntary backup push to `filefran.safespaceshub.com` |
| `/api/cloud/status` | `GET` | Verified Host | Retrieve current cloud sync status and last backup timestamp |
| `/api/admin/overview` | `GET` | Admin | System statistics, registered accounts, voluntary backups & active rooms |
| `/api/smtp/invite` | `POST` | Verified Host | Dispatch meeting invitation via `mail.safespaceshub.com` |
| `/api/smtp/config` | `POST` | Verified Host | Update SMTP credentials (preset: Stalwart / Custom) |
| `/api/status` | `GET` | Public | Room status, LAN IP, and shareable conference URLs |
| `/api/peers` | `GET` | Public | Active participant list and admission states |

---

## Project Structure

```
d:/c test - rust/
├── Cargo.toml                    # Standard Cargo package manifest (Zero external crates)
├── Dockerfile                    # Multi-stage container build for Grafana
├── logo.png                      # Official Safe Spaces AI Innovation Hub brand emblem
├── src/
│   ├── lib.rs                    # Public Rust library API & exports
│   ├── main.rs                   # Single unified appliance (SFU + HTTP + CLI + Engine)
│   ├── auth.rs                   # SHA-256 crypto, AuthRegistry, 6-digit codes & cloud sync
│   ├── error.rs                  # GrafanaError enum & Result type
│   ├── protocol.rs               # Wire packet framing, headers & Fletcher-32 checksum
│   ├── net.rs                    # Non-blocking UDP transport & MTU chunking
│   ├── peer.rs                   # PeerRegistry, PeerInfo & 15s dead-peer pruning
│   ├── consensus.rs              # Unanimous consent state machine & Veto engine
│   ├── recorder.rs               # Local disk sink with retroactive RIFF WAV header patching
│   ├── media.rs                  # Audio/Video frames, tracks & synchronized presentation
│   ├── smtp.rs                   # RFC 5321/5322 client for Stalwart Mail Server
│   ├── sfu.rs                    # In-memory stateless UDP SFU relay server
│   ├── http.rs                   # Embedded WebRTC UI HTTP server & REST API
│   ├── engine.rs                 # Core GrafanaEngine coordinator & poll event loop
│   └── bin/
│       └── grafana_relay.rs       # Standalone SFU relay server binary
├── web/
│   ├── index.html                # Clean UI with auth, cloud sync, admin & legal modals
│   ├── style.css                 # White and blue SaaS styling with Lucide SVG iconography and zero emojis
│   └── grafana_web.js             # Production WebRTC media capture, auth & sync client
└── README.md                     # Comprehensive architecture and compliance documentation
```

---

## Verification & Automated Test Suite

Run the full automated test suite:

```powershell
cargo test
```

All 26 unit tests verify:
- Cryptographic SHA-256 standard vectors & token generation
- Registration, 6-digit email confirmation lifecycle, and login authentication
- Unanimous consensus recording, veto mechanisms, and mid-call revocation
- Anti-capture permission checks and local WAV file recording
- Stalwart SMTP configuration masking, RFC 2822 formatting, and email validation
- In-memory SFU relay routing and peer registry heartbeat timeouts
