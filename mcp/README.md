# Safe Spaces AI Innovation Hub - Grafana MCP Server

This directory contains the standard Model Context Protocol (MCP) server for the **Safe Spaces AI Innovation Hub** Grafana Conference Platform.

It allows AI assistants (such as Antigravity, Claude Desktop, Cursor, Gemini CLI) to monitor, interact with, and orchestrate Grafana zero-cloud video conferences using standard JSON-RPC 2.0 stdio.

---

## Capabilities & Tools

| Tool | Purpose |
|---|---|
| `grafana_get_system_health` | Live operational health, uptime, active peer counts, routed SFU packets, vault persistence, and SMTP state |
| `grafana_get_peers` | Active participant list with admission and presence status |
| `grafana_join_room` | Join conference room as an agent participant |
| `grafana_admit_peer` | Admit or reject a peer waiting in the lobby |
| `grafana_get_consensus_status` | Status of the 100% Unanimous Consent recording ballot |
| `grafana_request_recording` | Initiate unanimous consent ballot for recording |
| `grafana_cast_vote` | Cast an approval or veto vote on an active recording ballot |
| `grafana_revoke_consent` | Instantly revoke consent and abort any in-progress call capture |
| `grafana_send_email_invite` | Dispatch invite email via the self-hosted Stalwart SMTP gateway |
| `grafana_test_smtp` | Diagnostic connectivity test and handshake with Stalwart mail server |
| `grafana_sync_cloud_vault` | Push encrypted vault payload to Safe Spaces Cloud (filefran.safespaceshub.com) |
| `grafana_get_admin_metrics` | View registered user accounts and cloud vault backup logs |

---

## Resources

- `grafana://health`: Live health telemetry and runtime metrics
- `grafana://consensus`: Active recording ballot status and votes count
- `grafana://peers`: Current conference room participant list

---

## Configuration

Add the following to your MCP client configuration (e.g. `claude_desktop_config.json` or Antigravity configuration):

```json
{
  "mcpServers": {
    "safespaces-grafana": {
      "command": "python",
      "args": ["d:\\c test - rust\\mcp\\grafana_mcp_server.py"],
      "env": {
        "GRAFANA_URL": "http://127.0.0.1:7878"
      }
    }
  }
}
```

---

## Testing the MCP Server

You can test the MCP server directly using standard JSON-RPC input:

```bash
python mcp/grafana_mcp_server.py
```

Send the initialize request:
```json
{"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}
```
The server will respond with supported tools and protocol details.
