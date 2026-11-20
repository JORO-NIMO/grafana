#!/usr/bin/env python3
"""
Model Context Protocol (MCP) Server for Safe Spaces AI Innovation Hub - Grafana Conference Platform

Exposes Grafana REST APIs, WebRTC controls, and 100% Unanimous Consent Engine
to AI agents over standard JSON-RPC 2.0 stdio.
"""

import sys
import json
import os
import urllib.request
import urllib.error

GRAFANA_BASE_URL = os.environ.get("GRAFANA_URL", "http://127.0.0.1:7878").rstrip("/")
DEFAULT_TOKEN = os.environ.get("GRAFANA_TOKEN", "")

TOOLS = [
    {
        "name": "grafana_get_system_health",
        "description": "Fetch live node health, uptime, active peer count, routed SFU packets, vault persistence status, and Stalwart SMTP state.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    },
    {
        "name": "grafana_get_peers",
        "description": "List all active participants and their admission status in the Grafana conference room.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "peer_id": {"type": "string", "description": "Optional caller peer ID to highlight self"}
            },
            "required": []
        }
    },
    {
        "name": "grafana_join_room",
        "description": "Join the conference room as an agent participant.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "name": {"type": "string", "description": "Agent display name (e.g. 'Antigravity AI Assistant')"},
                "peer_id": {"type": "string", "description": "Unique identifier for agent peer"}
            },
            "required": ["name"]
        }
    },
    {
        "name": "grafana_admit_peer",
        "description": "Admit or deny a peer waiting in the conference lobby.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "peer_id": {"type": "string", "description": "Peer ID in the lobby"},
                "admit": {"type": "boolean", "description": "True to admit into call, False to deny"}
            },
            "required": ["peer_id", "admit"]
        }
    },
    {
        "name": "grafana_get_consensus_status",
        "description": "Check current state of the 100% Unanimous Consent recording ballot.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    },
    {
        "name": "grafana_request_recording",
        "description": "Initiate a 100% unanimous consent ballot to request permission to record audio/video.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "requester_id": {"type": "string", "description": "ID of participant requesting recording"},
                "rec_type": {"type": "string", "enum": ["av", "audio"], "description": "Recording type: 'av' for full audio/video, 'audio' for voice only"}
            },
            "required": ["requester_id"]
        }
    },
    {
        "name": "grafana_cast_vote",
        "description": "Cast a vote on an active recording ballot. A single 'no' immediately blocks recording.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ballot_id": {"type": "integer", "description": "Active ballot ID"},
                "accept": {"type": "boolean", "description": "True to approve recording, False to veto"}
            },
            "required": ["ballot_id", "accept"]
        }
    },
    {
        "name": "grafana_revoke_consent",
        "description": "Instantly revoke recording consent and abort any in-progress call capture.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "ballot_id": {"type": "integer", "description": "Ballot ID to revoke"}
            },
            "required": ["ballot_id"]
        }
    },
    {
        "name": "grafana_send_email_invite",
        "description": "Dispatch a confidential meeting invitation via the self-hosted Stalwart SMTP gateway.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "to": {"type": "string", "description": "Attendee email address"},
                "name": {"type": "string", "description": "Attendee display name"},
                "note": {"type": "string", "description": "Confidential meeting note or agenda"},
                "token": {"type": "string", "description": "Authenticated user session token"}
            },
            "required": ["to"]
        }
    },
    {
        "name": "grafana_test_smtp",
        "description": "Perform diagnostic connectivity test and greeting handshake with Stalwart mail server.",
        "inputSchema": {
            "type": "object",
            "properties": {},
            "required": []
        }
    },
    {
        "name": "grafana_sync_cloud_vault",
        "description": "Save encrypted client vault payload to local disk ledger and push to Safe Spaces Cloud Vault (filefran.safespaceshub.com).",
        "inputSchema": {
            "type": "object",
            "properties": {
                "allow_sync": {"type": "boolean", "description": "User consent for cloud backup push"},
                "backup_payload": {"type": "object", "description": "Client vault backup data object"},
                "token": {"type": "string", "description": "Authenticated user session token"}
            },
            "required": ["token"]
        }
    },
    {
        "name": "grafana_get_admin_metrics",
        "description": "Administrator overview of registered user accounts and cloud vault backup logs.",
        "inputSchema": {
            "type": "object",
            "properties": {
                "token": {"type": "string", "description": "Administrator session token"}
            },
            "required": ["token"]
        }
    }
]

RESOURCES = [
    {
        "uri": "grafana://health",
        "name": "Grafana System Health",
        "description": "Live health telemetry and operational metrics",
        "mimeType": "application/json"
    },
    {
        "uri": "grafana://consensus",
        "name": "Consensus Ballot Status",
        "description": "Active recording ballot status and votes count",
        "mimeType": "application/json"
    },
    {
        "uri": "grafana://peers",
        "name": "Active Participants Roster",
        "description": "Current conference room participant list",
        "mimeType": "application/json"
    }
]

def http_req(endpoint, method="GET", data=None, token=None):
    url = f"{GRAFANA_BASE_URL}{endpoint}"
    req_token = token or DEFAULT_TOKEN
    headers = {"Content-Type": "application/json", "User-Agent": "Grafana-MCP-Server/1.2.0"}
    if req_token:
        headers["Authorization"] = f"Bearer {req_token}"

    body_bytes = None
    if data is not None:
        body_bytes = json.dumps(data).encode("utf-8")

    req = urllib.request.Request(url, data=body_bytes, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req, timeout=8) as resp:
            raw = resp.read().decode("utf-8")
            try:
                return json.loads(raw)
            except Exception:
                return {"raw": raw}
    except urllib.error.HTTPError as e:
        raw = e.read().decode("utf-8")
        try:
            err_json = json.loads(raw)
            return {"error": err_json, "status_code": e.code}
        except Exception:
            return {"error": raw, "status_code": e.code}
    except Exception as e:
        return {"error": str(e)}

def handle_tool_call(name, args):
    if name == "grafana_get_system_health":
        return http_req("/api/health")
    elif name == "grafana_get_peers":
        peer_id = args.get("peer_id", "")
        url = f"/api/peers?peer_id={peer_id}" if peer_id else "/api/peers"
        return http_req(url)
    elif name == "grafana_join_room":
        return http_req("/api/peers/join", "POST", args)
    elif name == "grafana_admit_peer":
        return http_req("/api/peers/admit", "POST", args)
    elif name == "grafana_get_consensus_status":
        return http_req("/api/consensus/status")
    elif name == "grafana_request_recording":
        return http_req("/api/consensus/request", "POST", args)
    elif name == "grafana_cast_vote":
        return http_req("/api/consensus/vote", "POST", args)
    elif name == "grafana_revoke_consent":
        return http_req("/api/consensus/revoke", "POST", args)
    elif name == "grafana_send_email_invite":
        token = args.get("token")
        return http_req("/api/smtp/invite", "POST", args, token=token)
    elif name == "grafana_test_smtp":
        return http_req("/api/smtp/test", "POST", {})
    elif name == "grafana_sync_cloud_vault":
        token = args.get("token")
        return http_req("/api/cloud/sync", "POST", args, token=token)
    elif name == "grafana_get_admin_metrics":
        token = args.get("token")
        return http_req("/api/admin/overview", "GET", token=token)
    else:
        return {"error": f"Unknown tool: {name}"}

def handle_resource_read(uri):
    if uri == "grafana://health":
        data = http_req("/api/health")
        return json.dumps(data, indent=2)
    elif uri == "grafana://consensus":
        data = http_req("/api/consensus/status")
        return json.dumps(data, indent=2)
    elif uri == "grafana://peers":
        data = http_req("/api/peers")
        return json.dumps(data, indent=2)
    else:
        return json.dumps({"error": f"Unknown resource URI: {uri}"})

def main():
    while True:
        line = sys.stdin.readline()
        if not line:
            break
        line = line.strip()
        if not line:
            continue

        try:
            req = json.loads(line)
        except Exception:
            continue

        msg_id = req.get("id")
        method = req.get("method")
        params = req.get("params", {})

        if method == "initialize":
            res = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "safespaces-grafana-mcp",
                        "version": "1.2.0"
                    },
                    "capabilities": {
                        "tools": {},
                        "resources": {}
                    }
                }
            }
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        elif method == "notifications/initialized":
            pass

        elif method == "ping":
            res = {"jsonrpc": "2.0", "id": msg_id, "result": {}}
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        elif method == "tools/list":
            res = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "tools": TOOLS
                }
            }
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        elif method == "tools/call":
            tool_name = params.get("name")
            tool_args = params.get("arguments", {})
            out = handle_tool_call(tool_name, tool_args)
            res = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "content": [
                        {
                            "type": "text",
                            "text": json.dumps(out, indent=2)
                        }
                    ]
                }
            }
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        elif method == "resources/list":
            res = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "resources": RESOURCES
                }
            }
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        elif method == "resources/read":
            uri = params.get("uri")
            content_text = handle_resource_read(uri)
            res = {
                "jsonrpc": "2.0",
                "id": msg_id,
                "result": {
                    "contents": [
                        {
                            "uri": uri,
                            "mimeType": "application/json",
                            "text": content_text
                        }
                    ]
                }
            }
            sys.stdout.write(json.dumps(res) + "\n")
            sys.stdout.flush()

        else:
            if msg_id is not None:
                res = {
                    "jsonrpc": "2.0",
                    "id": msg_id,
                    "error": {
                        "code": -32601,
                        "message": f"Method not found: {method}"
                    }
                }
                sys.stdout.write(json.dumps(res) + "\n")
                sys.stdout.flush()

if __name__ == "__main__":
    main()
