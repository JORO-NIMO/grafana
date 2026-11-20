//! OpenAPI 3.1.0 Specification & Interactive Documentation UI
//!
//! Safe Spaces AI Innovation Hub - Grafana Conference Platform

pub fn get_openapi_json() -> &'static str {
    r##"{
  "openapi": "3.1.0",
  "info": {
    "title": "Safe Spaces AI Innovation Hub - Grafana API",
    "version": "1.2.0",
    "description": "Production REST and WebRTC Signaling API for Safe Spaces AI Innovation Hub Grafana Conference Platform. Features 100% Unanimous Consent Recording, Zero Cloud Retention, Local Client Encrypted Vault Persistence with optional Safe Spaces Cloud Vault synchronization (filefran.safespaceshub.com), and Stalwart SMTP integration.",
    "contact": {
      "name": "Safe Spaces AI Innovation Hub Support",
      "url": "https://safespaceshub.com",
      "email": "support@safespaceshub.com"
    },
    "license": {
      "name": "MIT / Apache-2.0",
      "url": "https://safespaceshub.com/license"
    }
  },
  "servers": [
    {
      "url": "https://grafana.safespaceshub.com",
      "description": "Production Domain"
    },
    {
      "url": "http://localhost:7878",
      "description": "Local Development Server"
    }
  ],
  "tags": [
    { "name": "System", "description": "Node health, uptime, telemetry and hardware guard metrics" },
    { "name": "Participants", "description": "Peer presence, lobby management, profiles and heartbeat keepalive" },
    { "name": "Signaling", "description": "WebRTC SDP offer/answer exchange and ICE candidate polling" },
    { "name": "Consensus", "description": "100% Unanimous Consent Recording engine and instant revocation" },
    { "name": "Authentication", "description": "User registration, email verification, and salted session management" },
    { "name": "CloudSync", "description": "Client-encrypted vault persistence and optional push to filefran.safespaceshub.com" },
    { "name": "SMTP", "description": "Stalwart Mail Server gateway, diagnostic testing, and meeting invite dispatch" },
    { "name": "Admin", "description": "Administrative audit logging and encrypted sync monitoring" }
  ],
  "components": {
    "securitySchemes": {
      "BearerAuth": {
        "type": "http",
        "scheme": "bearer",
        "description": "Pass session token in Authorization header: Bearer <token>"
      },
      "XAuthToken": {
        "type": "apiKey",
        "in": "header",
        "name": "X-Auth-Token",
        "description": "Alternative authentication header"
      }
    }
  },
  "paths": {
    "/api/health": {
      "get": {
        "tags": ["System"],
        "summary": "Node health and runtime telemetry",
        "description": "Returns operational status, uptime in seconds, active peer counts, routed SFU packets, vault persistence status, and Stalwart SMTP state.",
        "responses": {
          "200": {
            "description": "System is healthy",
            "content": {
              "application/json": {
                "example": {
                  "status": "healthy",
                  "platform": "Safe Spaces AI Innovation Hub",
                  "service": "Grafana Conference Engine",
                  "version": "1.2.0",
                  "uptime_seconds": 3600,
                  "active_peers": 4,
                  "sfu_packets_routed": 18240,
                  "vault": { "storage": "atomic_disk_ledger", "file": "data/grafana_vault.json", "registered_users": 12 },
                  "smtp": { "configured": true, "host": "mail.safespaceshub.com", "port": 587 },
                  "cloud_sync": { "destination": "https://filefran.safespaceshub.com/api/v1/vault/sync", "mode": "user_opt_in" },
                  "local_ip": "192.168.1.50",
                  "network_url": "https://grafana.safespaceshub.com"
                }
              }
            }
          }
        }
      }
    },
    "/api/status": {
      "get": {
        "tags": ["System"],
        "summary": "Lightweight node status check",
        "responses": {
          "200": {
            "description": "Node is online",
            "content": {
              "application/json": {
                "example": {
                  "status": "online",
                  "platform": "Safe Spaces AI Innovation Hub",
                  "engine": "Grafana",
                  "version": "1.2.0",
                  "zero_data": true,
                  "sfu_packets": 18240,
                  "local_ip": "192.168.1.50",
                  "network_url": "https://grafana.safespaceshub.com"
                }
              }
            }
          }
        }
      }
    },
    "/api/stats": {
      "get": {
        "tags": ["System"],
        "summary": "Media routing and SFU relay statistics",
        "responses": {
          "200": {
            "description": "Detailed media metrics",
            "content": {
              "application/json": {
                "example": {
                  "status": "online",
                  "engine": "Grafana-Rust",
                  "version": "1.2.0",
                  "zero_data": true,
                  "sfu_packets": 18240,
                  "server_disk_writes_bytes": 0,
                  "active_rooms": 1,
                  "active_peers": 3,
                  "consensus_rule": "100% Unanimous Agreement Required"
                }
              }
            }
          }
        }
      }
    },
    "/api/anticapture/status": {
      "get": {
        "tags": ["System"],
        "summary": "Hardware anti-capture guard status",
        "responses": {
          "200": {
            "description": "Anti-capture protection armed",
            "content": {
              "application/json": {
                "example": {
                  "guard_active": true,
                  "screenshots_blocked": true,
                  "bg_device_recording_blocked": true,
                  "unanimous_consent_required": true,
                  "status": "ARMED"
                }
              }
            }
          }
        }
      }
    },
    "/api/peers": {
      "get": {
        "tags": ["Participants"],
        "summary": "List active peers in the room",
        "parameters": [
          { "name": "peer_id", "in": "query", "description": "Caller peer ID to mark is_self", "required": false, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": {
            "description": "Active participant roster",
            "content": {
              "application/json": {
                "example": {
                  "room": "Grafana-Secure-Room",
                  "auto_admit": true,
                  "peers": [
                    { "id": "user_49102", "name": "Alice", "avatar": "", "status": "Admitted", "is_self": true, "joined_sec": 1700000000 }
                  ]
                }
              }
            }
          }
        }
      }
    },
    "/api/peers/join": {
      "post": {
        "tags": ["Participants"],
        "summary": "Join the conference room or lobby",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "peer_id": "user_49102", "name": "Alice", "avatar": "" }
            }
          }
        },
        "responses": {
          "200": { "description": "Peer admitted or placed in waiting lobby" }
        }
      }
    },
    "/api/peers/admit": {
      "post": {
        "tags": ["Participants"],
        "summary": "Admit or deny a peer in the lobby",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "peer_id": "user_49102", "admit": true }
            }
          }
        },
        "responses": {
          "200": { "description": "Admission decision applied" }
        }
      }
    },
    "/api/peers/profile": {
      "post": {
        "tags": ["Participants"],
        "summary": "Update participant display name or avatar",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "peer_id": "user_49102", "name": "Alice M.", "avatar": "" }
            }
          }
        },
        "responses": {
          "200": { "description": "Profile updated" }
        }
      }
    },
    "/api/peers/heartbeat": {
      "post": {
        "tags": ["Participants"],
        "summary": "Send keepalive heartbeat",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": { "example": { "peer_id": "user_49102" } }
          }
        },
        "responses": {
          "200": { "description": "Heartbeat touched" }
        }
      }
    },
    "/api/peers/leave": {
      "post": {
        "tags": ["Participants"],
        "summary": "Depart from the conference room",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": { "example": { "peer_id": "user_49102" } }
          }
        },
        "responses": {
          "200": { "description": "Left successfully" }
        }
      }
    },
    "/api/signal/send": {
      "post": {
        "tags": ["Signaling"],
        "summary": "Send WebRTC SDP offer/answer or ICE candidate",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": {
                "from": "user_1",
                "to": "user_2",
                "type": "offer",
                "payload": { "type": "offer", "sdp": "v=0..." }
              }
            }
          }
        },
        "responses": {
          "200": { "description": "Signaling packet queued for recipient" }
        }
      }
    },
    "/api/signal/poll": {
      "get": {
        "tags": ["Signaling"],
        "summary": "Poll pending signaling packets for caller",
        "parameters": [
          { "name": "peer_id", "in": "query", "required": true, "schema": { "type": "string" } }
        ],
        "responses": {
          "200": { "description": "Array of pending signaling messages" }
        }
      }
    },
    "/api/consensus/request": {
      "post": {
        "tags": ["Consensus"],
        "summary": "Initiate 100% Unanimous Consent ballot for recording",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "requester_id": "user_49102", "rec_type": "av" }
            }
          }
        },
        "responses": {
          "200": { "description": "Ballot created and broadcast to all peers" }
        }
      }
    },
    "/api/consensus/vote": {
      "post": {
        "tags": ["Consensus"],
        "summary": "Cast vote on active recording ballot",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "ballot_id": 105, "accept": true }
            }
          }
        },
        "responses": {
          "200": { "description": "Vote counted; 1 single 'no' terminates ballot immediately" }
        }
      }
    },
    "/api/consensus/revoke": {
      "post": {
        "tags": ["Consensus"],
        "summary": "Instantly revoke recording consent and abort active capture",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "ballot_id": 105 }
            }
          }
        },
        "responses": {
          "200": { "description": "Consent revoked and recording halted" }
        }
      }
    },
    "/api/consensus/status": {
      "get": {
        "tags": ["Consensus"],
        "summary": "Query current consensus ballot status",
        "responses": {
          "200": { "description": "Ballot state, votes count, and requirement" }
        }
      }
    },
    "/api/auth/register": {
      "post": {
        "tags": ["Authentication"],
        "summary": "Register new account for hosting meetings",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "username": "dr_smith", "email": "smith@safespaceshub.com", "password": "SecretPassword123" }
            }
          }
        },
        "responses": {
          "200": { "description": "Account created; 6-digit confirmation code dispatched via Stalwart SMTP" },
          "400": { "description": "Username/email already in use or short password" }
        }
      }
    },
    "/api/auth/verify": {
      "post": {
        "tags": ["Authentication"],
        "summary": "Verify email with 6-digit confirmation code",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "email": "smith@safespaceshub.com", "code": "847291" }
            }
          }
        },
        "responses": {
          "200": { "description": "Email verified; session token issued" },
          "400": { "description": "Invalid confirmation code" }
        }
      }
    },
    "/api/auth/login": {
      "post": {
        "tags": ["Authentication"],
        "summary": "Sign in with username/email and password",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": { "identifier": "dr_smith", "password": "SecretPassword123" }
            }
          }
        },
        "responses": {
          "200": { "description": "Login successful; session token returned" },
          "401": { "description": "Invalid credentials" },
          "403": { "description": "Email not confirmed" }
        }
      }
    },
    "/api/auth/me": {
      "get": {
        "tags": ["Authentication"],
        "summary": "Inspect current logged in user session",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "responses": {
          "200": { "description": "Session details and permissions" }
        }
      }
    },
    "/api/auth/logout": {
      "post": {
        "tags": ["Authentication"],
        "summary": "Invalidate current session token",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "responses": {
          "200": { "description": "Session invalidated" }
        }
      }
    },
    "/api/cloud/sync": {
      "post": {
        "tags": ["CloudSync"],
        "summary": "Save and dispatch encrypted vault backup to Safe Spaces Cloud",
        "description": "Records user preferences and client-encrypted vault data in the atomic disk ledger, and pushes to filefran.safespaceshub.com when opted-in.",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": {
                "allow_sync": true,
                "backup_payload": { "meeting_notes": "Confidential research notes", "client_timestamp": 1700000000 }
              }
            }
          }
        },
        "responses": {
          "200": { "description": "Vault updated locally and pushed to Safe Spaces Cloud Vault" }
        }
      }
    },
    "/api/cloud/status": {
      "get": {
        "tags": ["CloudSync"],
        "summary": "Check vault sync state and destination",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "responses": {
          "200": { "description": "Sync preference and timestamp" }
        }
      }
    },
    "/api/smtp/status": {
      "get": {
        "tags": ["SMTP"],
        "summary": "View masked SMTP gateway configuration",
        "responses": {
          "200": { "description": "Configured host, port, and sender address" }
        }
      }
    },
    "/api/smtp/config": {
      "post": {
        "tags": ["SMTP"],
        "summary": "Update volatile SMTP settings",
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": {
                "host": "mail.safespaceshub.com",
                "port": 587,
                "username": "grafana@safespaceshub.com",
                "password": "Password123",
                "sender_email": "grafana@safespaceshub.com",
                "sender_name": "Safe Spaces AI Innovation Hub"
              }
            }
          }
        },
        "responses": {
          "200": { "description": "Configuration updated in memory" }
        }
      }
    },
    "/api/smtp/test": {
      "post": {
        "tags": ["SMTP"],
        "summary": "Run diagnostic connectivity test to Stalwart mail server",
        "responses": {
          "200": { "description": "Diagnostic handshake result and log" }
        }
      }
    },
    "/api/smtp/invite": {
      "post": {
        "tags": ["SMTP"],
        "summary": "Dispatch meeting invitation email",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "requestBody": {
          "required": true,
          "content": {
            "application/json": {
              "example": {
                "to": "colleague@safespaceshub.com",
                "name": "Dr. Watson",
                "note": "Reviewing security architecture and privacy policies."
              }
            }
          }
        },
        "responses": {
          "200": { "description": "Invite dispatched with RFC 5322 MIME formatting" }
        }
      }
    },
    "/api/admin/overview": {
      "get": {
        "tags": ["Admin"],
        "summary": "Administrator view of registered users and cloud backup logs",
        "security": [{ "BearerAuth": [] }, { "XAuthToken": [] }],
        "responses": {
          "200": { "description": "Audit records and sync telemetry" },
          "403": { "description": "Admin privileges required" }
        }
      }
    }
  }
}"##
}

pub fn get_api_docs_html() -> &'static str {
    r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>API Reference - Safe Spaces AI Innovation Hub</title>
  <style>
    :root {
      --bg: #07090e;
      --card-bg: #0f141f;
      --border: rgba(255, 255, 255, 0.08);
      --cyan: #00f2fe;
      --blue: #4facfe;
      --text: #f1f5f9;
      --text-muted: #94a3b8;
      --get-badge: #10b981;
      --post-badge: #0284c7;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Helvetica, Arial, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.5;
      padding: 24px;
    }
    .container { max-width: 1100px; margin: 0 auto; }
    header {
      border-bottom: 1px solid var(--border);
      padding-bottom: 24px;
      margin-bottom: 32px;
      display: flex;
      flex-wrap: wrap;
      justify-content: space-between;
      align-items: center;
      gap: 16px;
    }
    h1 { font-size: 26px; font-weight: 700; color: #fff; display: flex; align-items: center; gap: 10px; }
    .tagline { color: var(--text-muted); font-size: 14px; margin-top: 6px; }
    .btn-download {
      background: rgba(0, 242, 254, 0.1);
      border: 1px solid var(--cyan);
      color: var(--cyan);
      padding: 8px 18px;
      border-radius: 8px;
      font-weight: 600;
      font-size: 13px;
      text-decoration: none;
      transition: all 0.2s;
    }
    .btn-download:hover { background: var(--cyan); color: #000; }
    .section-title {
      font-size: 18px;
      font-weight: 700;
      color: var(--cyan);
      margin: 28px 0 14px 0;
      text-transform: uppercase;
      letter-spacing: 0.5px;
    }
    .endpoint {
      background: var(--card-bg);
      border: 1px solid var(--border);
      border-radius: 10px;
      margin-bottom: 12px;
      overflow: hidden;
      transition: border-color 0.2s;
    }
    .endpoint:hover { border-color: rgba(0, 242, 254, 0.3); }
    .endpoint-header {
      padding: 14px 18px;
      display: flex;
      align-items: center;
      gap: 14px;
      cursor: pointer;
      user-select: none;
    }
    .badge {
      font-size: 12px;
      font-weight: 700;
      padding: 4px 10px;
      border-radius: 6px;
      color: #fff;
      min-width: 54px;
      text-align: center;
    }
    .badge-get { background: var(--get-badge); }
    .badge-post { background: var(--post-badge); }
    .path { font-family: monospace; font-size: 14px; font-weight: 600; color: #fff; }
    .desc { font-size: 13px; color: var(--text-muted); margin-left: auto; }
    .endpoint-body {
      padding: 18px;
      border-top: 1px solid var(--border);
      background: rgba(0, 0, 0, 0.25);
      display: none;
    }
    .endpoint-body.open { display: block; }
    pre {
      background: #06090e;
      border: 1px solid var(--border);
      border-radius: 6px;
      padding: 12px;
      font-family: monospace;
      font-size: 12px;
      color: #38bdf8;
      overflow-x: auto;
      margin: 8px 0 14px 0;
    }
    .btn-try {
      background: linear-gradient(135deg, var(--cyan), var(--blue));
      color: #030712;
      border: none;
      padding: 6px 16px;
      border-radius: 6px;
      font-weight: 700;
      font-size: 12px;
      cursor: pointer;
    }
    .result-box {
      margin-top: 10px;
      display: none;
    }
  </style>
</head>
<body>
  <div class="container">
    <header>
      <div>
        <h1>Safe Spaces AI Innovation Hub</h1>
        <p class="tagline">Grafana Conference Platform &bull; OpenAPI 3.1.0 Specification &bull; Zero Cloud Retention &bull; Pure Rust</p>
      </div>
      <div>
        <a class="btn-download" href="/api/openapi.json" target="_blank">Download openapi.json</a>
      </div>
    </header>

    <div class="section-title">System & Telemetry</div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/health</span>
        <span class="desc">System health, uptime, vault storage, and SMTP connectivity</span>
      </div>
      <div class="endpoint-body">
        <p style="font-size:13px; color:#cbd5e1;">Returns node operational health, server uptime in seconds, vault status, and Stalwart SMTP config.</p>
        <button class="btn-try" onclick="tryApi('/api/health', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/status</span>
        <span class="desc">Lightweight node status</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/status', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/stats</span>
        <span class="desc">WebRTC SFU routed packets and peer counts</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/stats', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>

    <div class="section-title">Lobby & Participants</div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/peers</span>
        <span class="desc">Active participants list</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/peers', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-post">POST</span>
        <span class="path">/api/peers/join</span>
        <span class="desc">Join conference room</span>
      </div>
      <div class="endpoint-body">
        <p style="font-size:12px; color:#94a3b8;">Payload:</p>
        <pre>{"peer_id": "test_peer", "name": "API Tester"}</pre>
        <button class="btn-try" onclick="tryApi('/api/peers/join', 'POST', JSON.stringify({peer_id:'test_peer', name:'API Tester'}), this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>

    <div class="section-title">Consensus Engine (100% Unanimous Agreement)</div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/consensus/status</span>
        <span class="desc">Check recording ballot state</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/consensus/status', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-post">POST</span>
        <span class="path">/api/consensus/revoke</span>
        <span class="desc">Instant unilateral recording revocation</span>
      </div>
      <div class="endpoint-body">
        <p style="font-size:12px; color:#94a3b8;">Revokes consent immediately and aborts capture without confirmation.</p>
        <button class="btn-try" onclick="tryApi('/api/consensus/revoke', 'POST', JSON.stringify({ballot_id: 0}), this)">Execute Revoke</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>

    <div class="section-title">Authentication & Vault Synchronization</div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/auth/me</span>
        <span class="desc">Inspect current logged in user</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/auth/me', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/cloud/status</span>
        <span class="desc">Check cloud vault backup settings</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/cloud/status', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>

    <div class="section-title">Stalwart Mail Server Gateway</div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-get">GET</span>
        <span class="path">/api/smtp/status</span>
        <span class="desc">View masked SMTP gateway configuration</span>
      </div>
      <div class="endpoint-body">
        <button class="btn-try" onclick="tryApi('/api/smtp/status', 'GET', null, this)">Execute Test</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
    <div class="endpoint">
      <div class="endpoint-header" onclick="toggle(this)">
        <span class="badge badge-post">POST</span>
        <span class="path">/api/smtp/test</span>
        <span class="desc">Execute diagnostic test against Stalwart server</span>
      </div>
      <div class="endpoint-body">
        <p style="font-size:13px; color:#cbd5e1;">Initiates a direct handshake with the configured SMTP server and verifies greeting.</p>
        <button class="btn-try" onclick="tryApi('/api/smtp/test', 'POST', '{}', this)">Execute SMTP Diagnostic</button>
        <div class="result-box"><pre></pre></div>
      </div>
    </div>
  </div>

  <script>
    function toggle(header) {
      const body = header.nextElementSibling;
      body.classList.toggle('open');
    }

    async function tryApi(url, method, body, btn) {
      const container = btn.parentElement.querySelector('.result-box');
      const pre = container.querySelector('pre');
      container.style.display = 'block';
      pre.textContent = 'Executing request to ' + url + '...';

      try {
        const opts = { method: method, headers: {} };
        if (body && method === 'POST') {
          opts.headers['Content-Type'] = 'application/json';
          opts.body = body;
        }
        const res = await fetch(url, opts);
        const data = await res.json().catch(() => res.text());
        pre.textContent = 'HTTP ' + res.status + ' ' + res.statusText + '\n\n' +
          (typeof data === 'object' ? JSON.stringify(data, null, 2) : data);
      } catch (err) {
        pre.textContent = 'Error: ' + err.message;
      }
    }
  </script>
</body>
</html>"##
}
