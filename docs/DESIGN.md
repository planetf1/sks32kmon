# SKS3200 Switch Manager — Design Document

**Date:** 2026-07-22
**Status:** Draft
**Author:** Engineering Pod

---

## 1. Problem

Two XikeStor (兮克) SKS3200-8E2X switches live on the LAN at `192.168.100.7` and
`192.168.100.8`. They have a basic web dashboard (login → HTML forms + JSON
API) but no CLI, no SNMP, no SSH, and no modern API surface. Managing them
requires opening a browser, logging in, and clicking through pages.

We need a Rust-based CLI + TUI tool to query switch state **read-only** (Phase 1),
with a clear path to write support (Phase 2).

### Switch Hardware

| Spec | Value |
|---|---|
| Model | SKS3200-8E2X |
| Brand | XikeStor (兮克) |
| Ports | 8 × 2.5G RJ45 + 2 × 10G SFP+ |
| Chipset | MaxLinear MXL86282S |
| Firmware | 2.0.0.3 |
| Web UI | Bootstrap 5, static HTML + Axios JSON API |
| Auth | GET `/authorize` with MD5(username) + MD5(password) |
| Session | Cookie-based (`session`, `user`) |

---

## 2. Architecture

### 2.1 Crate Structure (single crate)

```
sks3200/
├── Cargo.toml
├── docs/
│   └── DESIGN.md
├── src/
│   ├── main.rs            # CLI entry point (clap)
│   ├── client.rs           # HTTP client: auth, session, GET requests
│   ├── models.rs           # Serde-deserializable API response types
│   └── tui.rs              # Ratatui TUI dashboard (feature-gated)
```

### 2.2 Dependencies

| Crate | Use |
|---|---|
| `clap` + `clap-verbosity-flag` | CLI arg parsing |
| `reqwest` (blocking, `cookies`) | HTTP client with cookie store |
| `serde` + `serde_json` | JSON deserialization |
| `serde_derive` | Derive macros |
| `ratatui` + `crossterm` | TUI (feature-gated: `tui`) |
| `chrono` | Timestamps |
| `anyhow` | Error handling |

### 2.3 Feature Flags

```toml
[features]
default = ["cli"]
cli = ["dep:clap"]
tui = ["dep:ratatui", "dep:crossterm"]
```

---

## 3. API Surface (Discovered)

All endpoints are relative to `http://<switch-ip>/`. Authentication requires a
valid session cookie obtained via `GET /authorize`.

### 3.1 Read-Only Endpoints

| Endpoint | Response Model | Notes |
|---|---|---|
| `GET /status.json` | `SystemInfo` | Temperature, IP, MAC, FW/HW version, description |
| `GET /port_setting_load.json` | `PortSettings` | Per-port: status, speed/duplex, flow control, EEE |
| `GET /port_statistics.json` | `PortStatistics` | Per-port: Tx/Rx good/bad packets |
| `GET /mac_get_dynamic_mac_entries.json` | `Vec<MacEntry>` | Dynamic MAC table (paginated, `data:` prefix) |
| `GET /mac_get_static_mac_entries.json` | `Vec<StaticMacEntry>` | Static MAC entries |
| `GET /port_trunk_cfg.json` | `TrunkConfig` | Link aggregation groups, LACP config |
| `GET /port_loop_status.json` | `LoopStatus` | Per-port loop violation detect |
| `GET /port_lock_cfg.json` | `LoopProtectionConfig` | Loop protection settings |
| `GET /stp.json` | `StpConfig` | STP/RSTP enable, per-port status |
| `GET /port_vlan.json` | `PortVlanConfig` | Per-port PVID, frame type |
| `GET /tag_vlan.json` | `TagVlanConfig` | Tagged VLAN config |
| `GET /all_port_pvid.json` | `PortPvids` | All port PVIDs in compact form |
| `GET /storm_ctrl_cfg.json` | `StormControlConfig` | Per-port storm control rates |
| `GET /port_mirror.json` | `PortMirrorConfig` | Port mirroring settings |
| `GET /igmp_config.json` | `IgmpConfig` | IGMP snooping, fast leave, report flood |
| `GET /network_settings.json` | `NetworkSettings` | IP, netmask, gateway, DHCP, DNS |
| `GET /systemtime_settings.json` | `SystemTimeSettings` | System time config |

### 3.2 Write Endpoints (Phase 2, documented for reference)

| Endpoint | Method | Purpose |
|---|---|---|
| `POST /save_all_configs.json` | POST | Save running config to startup |
| `POST /apply_user_port_setting.json` | POST | Apply port settings |
| `POST /set_des.json` | POST | Set device description |
| `POST /system_reboot.json` | POST | Reboot switch |
| `POST /network_settings_ipv4.json` | POST | Update IPv4 settings |

---

## 4. Data Models

### 4.1 SystemInfo

```rust
struct SystemInfo {
    temperature: String,  // "53"
    sys_ipv4: String,     // "192.168.100.7"
    sys_macaddr: String,  // "8C:A6:82:71:8F:B4"
    fw_ver: String,       // "2.0.0.3"
    hw_ver: String,       // "A0"
    des: String,          // "SKS3200-8E2X"
}
```

### 4.2 Port (common pattern)

Ports are indexed `Port_1` through `Port_10` (for 8+2 mode). Fields vary by
endpoint but share common structure:

| Field | Example | Endpoints |
|---|---|---|
| `Port_Id` | `"1"` | All |
| `Port_Status` | `"Enabled"` / `"Disabled"` | Settings, Statistics |
| `Spd_Duplex_Actual` | `"2500MbpsFull"` / `"Link Down"` | Settings |
| `Link_Status` | `"2500MbpsFull"` / `"Link Down"` | Statistics |

### 4.3 MAC Entry

```rust
struct MacEntry {
    dynamic_idx: u32,
    dynamic_mac_addr: String,   // "00:0E:58:85:04:82"
    dynamic_vlan_id: u32,
    dynamic_fid: u32,
    dynamic_portid: u32,
    dynamic_age_timer: u32,
}
```

---

## 5. CLI Design (clap)

```text
sks3200 [--switch <IP|HOST>] [--user <USER>] [--password <PASS>] <COMMAND>

Commands:
  status          System information (temperature, IP, MAC, FW)
  ports           Port status and settings
  statistics      Port traffic statistics (live with --watch)
  mac             Dynamic MAC address table
  trunk           Link aggregation / trunk status
  vlan            VLAN configuration
  stp             Spanning Tree Protocol status
  loop            Loop protection status
  all             Show everything
  monitor         TUI dashboard (if built with `tui` feature)

Global options:
  -s, --switch <IP>     Switch IP address (default: 192.168.100.7)
  -u, --user <USER>     Username (default: admin)
  -p, --password <PASS> Password (env: SKS3200_PASSWORD)
  -j, --json            Output raw JSON instead of formatted text
```

---

## 6. TUI Design (Phase 1.5)

A ratatui-based dashboard showing:

```
┌───────── SKS3200-8E2X @ 192.168.100.7 ─────────────────────┐
│ Firmware: 2.0.0.3  HW: A0  MAC: 8C:A6:82:71:8F:B4  53°C  │
├─────────── Ports ───────────────────────────────────────────┤
│ Port  Status     Speed       TxPkts       RxPkts      Flow  │
│  1    Enabled    Link Down        0            0       On   │
│  2    Enabled    2500Mbps  78,925,817  24,200,057     On   │
│  3    Enabled    100Mbps    4,109,328     709,064     On   │
│ ...                                                         │
├─────────── MAC Table (37 entries) ──────────────────────────┤
│ MAC               VLAN  Port  Age                          │
│ 00:0E:58:85:04:82    1     8   244s                        │
│ BC:24:11:09:EF:39    1     8   300s                        │
│ ...                                                         │
└─────────────────────────────────────────────────────────────┘
```

Features:
- Top pane: system info, refresh every 5s
- Middle pane: port summary table, refresh every 2s
- Bottom pane: MAC table or detail view
- Tab/arrow navigation between panes
- `q` to quit, `r` to force refresh

---

## 7. Future Work (Phase 2)

- **Write support:** Port enable/disable, VLAN config, link aggregation setup
- **Config backup:** Download/upload config files
- **Firmware upgrade:** Trigger via API
- **SNMP proxy:** Expose switch metrics via SNMP
- **Multi-switch aggregate view:** Dashboard across both switches

---

## 8. Security Notes

- Credentials transmitted as MD5 hashes (which is reversible for short/common
  passwords — treat as plaintext equivalent on the wire)
- Session cookie is `http-only` but over plain HTTP
- No CSRF protection observed
- **Recommendation:** Run management tool on a management VLAN; never expose
  switch web UI to WAN

---

## Appendix A: Discovery Methodology

1. Fetch `GET /login.html` → identified auth mechanism
2. Compute MD5 of creds → authenticate via `GET /authorize`
3. Capture session cookie
4. Fetch all HTML pages to extract inline JS `axios({url: '...'})` calls
5. For each discovered URL, issue `GET` with session cookie
6. Record response JSON structure
