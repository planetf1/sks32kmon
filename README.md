# sks3200 — XikeStor SKS3200-8E2X Switch Manager

A Rust CLI + TUI tool for managing **XikeStor (兮克) SKS3200-8E2X** switches
remotely. These are 8×2.5G + 2×10G SFP+ web-managed switches with a basic
HTML dashboard — no CLI, no SSH, no SNMP.

**Phase 1: Read-only.** Query system info, port status, traffic statistics,
MAC tables, VLANs, STP, loop protection, and more from the terminal.

---

## Quick start

```bash
# Generate a config file template and edit it
sks3200 config-init > ~/.config/sks3200/config.toml
# Then edit ~/.config/sks3200/config.toml with your switch credentials

# Once configured, query all switches at once
sks3200 status

# Port status
sks3200 ports

# Traffic statistics (live refresh)
sks3200 statistics --watch

# MAC address table
sks3200 mac

# Everything at once
sks3200 all

# JSON output for scripting
sks3200 -j status | jq '.temperature'
```

## Example output

All commands produce colour-coded, aligned tables with live data.

### `sks3200 status`

```
═══ main (192.168.100.7) ═══
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 SKS3200-8E2X  SKS3200-8E2X
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
  Firmware:           2.0.0.3
  Hardware:           A0
  MAC Address:        AA:BB:CC:DD:EE:FF
  IP Address:         192.168.100.7
  Temperature:        52°C
  Netmask:            255.255.255.0
  Gateway:            192.168.100.254
  DNS:                8.8.8.8
  DHCP:               Static
```

### `sks3200 ports`

```
═══ main (192.168.100.7) ═══
 PORT SETTINGS  Mode: PORT_MODE_8_PLUS_2  Active: 6/10
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Port    Status     Actual Speed         Config               Flow Ctrl    EEE
──────────────────────────────────────────────────────────────────────────────────────────
 Port 1  Enabled    Link Down            Auto                 On           Inactive
 Port 2  Enabled    2500MbpsFull         Auto                 On           Inactive
 Port 3  Enabled    100MbpsFull          Auto                 Off          Inactive
 Port 4  Enabled    1000MbpsFull         Auto                 On           Inactive
 Port 5  Enabled    Link Down            Auto                 On           Inactive
 Port 6  Enabled    1000MbpsFull         Auto                 On           Active
 Port 7  Enabled    100MbpsFull          Auto                 On           Inactive
 Port 8  Enabled    2500MbpsFull         Auto                 On           Active
 Port 9  Enabled    Link Down            Auto                 Off          N/A
 Port 10 Enabled    Link Down            Auto                 Off          N/A
```

### `sks3200 statistics`

```
═══ main (192.168.100.7) ═══
 PORT STATISTICS
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Port    Status           Tx Good          Tx Bad           Rx Good          Rx Bad
────────────────────────────────────────────────────────────────────────────────────────────────────
 Port 1  Link Down        0                0                0                0
 Port 2  2500MbpsFull     80.4M            0                25.2M            5
 Port 3  100MbpsFull      4.2M             0                737.5K           0
 Port 4  1000MbpsFull     143.8M           0                9.9M             0
 Port 5  Link Down        0                0                0                0
 Port 6  1000MbpsFull     3.7M             0                86.2K            11.8K
 Port 7  100MbpsFull      4.0M             0                1.2M             0
 Port 8  2500MbpsFull     54.5M            0                221.4M           0
 Port 9  Link Down        0                0                0                0
 Port 10 Link Down        0                0                0                0
```

Add `--watch` for a live-updating view (refreshes every 2s).

### `sks3200 mac`

```
═══ main (192.168.100.7) ═══
 DYNAMIC MAC TABLE  37 entries
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 #   MAC Address            VLAN   Port   Age
──────────────────────────────────────────────────────────────────────────────────────────
 1   00:1A:2B:3C:4D:01      1      8      225   s
 2   00:1A:2B:3C:4D:02      1      6      300   s
 3   00:1A:2B:3C:4D:03      1      8      300   s
 4   00:1A:2B:3C:4D:04      1      8      188   s
 5   00:1A:2B:3C:4D:05      1      2      300   s
 6   00:1A:2B:3C:4D:06      1      8      281   s
 ...
 37  AA:BB:CC:DD:EE:FF      1      0      225   s     ← the switch itself
```

### `sks3200 vlan`

```
═══ main (192.168.100.7) ═══
 PORT VLAN CONFIGURATION
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Port    PVID     Frame Type
──────────────────────────────────────────────────
 Port 1  1        All
 Port 2  1        All
 Port 3  1        All
 Port 4  1        All
 Port 5  1        All
 Port 6  1        All
 Port 7  1        All
 Port 8  1        All
 Port 9  1        All
 Port 10 1        All
```

### `sks3200 stp`

```
═══ main (192.168.100.7) ═══
 SPANNING TREE  Mode: RSTP  (Disabled)
╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌
 Port    Status       Edge     Path
────────────────────────────────────────────────────────────
 Port 1  Disabled     No       ─
 Port 2  Disabled     No       ─
 Port 3  Disabled     No       ─
 Port 4  Disabled     No       ─
 Port 5  Disabled     No       ─
 Port 6  Disabled     No       ─
 Port 7  Disabled     No       ─
 Port 8  Disabled     No       ─
 Port 9  Disabled     No       ─
 Port 10 Disabled     No       ─
```

### `sks3200 all`

Runs all supported commands in sequence and prints every section — useful for a
quick full inventory of a switch. Output combines `status`, `ports`,
`statistics`, `mac`, `vlan`, `network`, `loop`, and `stp` sections.

### TUI dashboard (`sks3200 monitor`)

A ratatui-based real-time dashboard with auto-refresh.
```
┌──────────── Status ────────────┬────────── Ports ───────────┐
│ SKS3200-8E2X v2.0.0.3         │ Port  Speed     Rx       Tx│
│ MAC: AA:BB:CC:DD:EE:FF        │  1    DOWN      0        0 │
│ IP:  192.168.100.7            │  2  2.5G  25.2M   80.4M   │
│ Temp: 52°C                    │  3  100M  737.5K   4.2M   │
│ DHCP: Static                  │  4   1G    9.9M  143.8M   │
├─────────── All sections ───────┤  ...                        │
│ Use Tab to cycle:             │  8  2.5G 221.4M  54.5M    │
│  • Status                     │  9  DOWN      0        0   │
│  • Ports                      │ 10  DOWN      0        0   │
│  • Statistics                 └─────────────────────────────┘
│  • MAC Table
│  • STP / VLAN / Network
│  • Loop / IGMP / Storm        Keys: q=quit, +/-=refresh rate
└───────────────────────────────┘
```

Run it:
```bash
sks3200 -s 192.168.100.7 monitor
```

Keys: `q` to quit, `Tab` to cycle sections, `+`/`-` to adjust refresh rate
(1–60 s), arrows to scroll.

## Commands

| Command | Description |
|---|---|
| `status` | System information (firmware, IP, MAC, temperature) |
| `ports` | Port status, speed, flow control, EEE |
| `statistics [--watch]` | Port traffic counters (live every 2s) |
| `mac` | Dynamic MAC address table |
| `static-mac` | Static MAC entries |
| `trunk` | Link aggregation / LACP status |
| `vlan` | Port VLAN (PVID, frame type) |
| `stp` | Spanning Tree Protocol status (per-port) |
| `loop` | Loop protection violation status |
| `config-init` | Generate a sample config file template |
| `igmp` | IGMP snooping configuration |
| `storm` | Storm control settings |
| `mirror` | Port mirroring configuration |
| `network` | IP, gateway, DNS settings |
| `all` | Everything in one view |
| `monitor` | TUI dashboard (requires `tui` feature) |

## Options

| Flag | Env | Default | Description |
|---|---|---|---|
| `-s, --switch` | `SKS3200_HOST` | — (all configured) | Switch names/IPs (comma-separated) |
| `-c, --config` | — | `~/.config/sks3200/config.toml` | Path to config file |
| `-u, --user` | `SKS3200_USER` | `admin` | Login username |
| `-p, --password` | `SKS3200_PASSWORD` | — | Login password |
| `-j, --json` | — | — | Output raw JSON |

## Multi-switch

Configure multiple switches in `~/.config/sks3200/config.toml` (generate a
template with `sks3200 config-init`). Commands query all configured switches
by default, with a `═══ name (host) ═══` header between them.

```toml
[[switch]]
name = "main"
host = "192.168.100.7"
password = "changeme"

[[switch]]
name = "secondary"
host = "192.168.100.8"
password = "changeme"
```

Target specific switches with `-s`:

```bash
sks3200 -s main status
sks3200 -s secondary ports
sks3200 -s 192.168.100.7 -- all   # ad-hoc (not in config)
```

## Installation

**From source:**
```bash
cargo install --path .
```

**From source with TUI dashboard:**
```bash
cargo install --features tui --path .
```

**Via Homebrew (macOS / Linux):**
```bash
brew install planetf1/tap/sks3200
```

**Via installer script (any platform):**
```bash
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/planetf1/sks32kmon/releases/latest/download/sks3200-installer.sh | sh
```

> Binaries are published for Apple Silicon, Intel Mac, ARM64 Linux, and x86_64
> Linux on the [GitHub Releases page](https://github.com/planetf1/sks32kmon/releases).

## Security

- Credentials are transmitted as MD5 hashes (plaintext-equivalent on the wire).
- Session cookies use `http-only` over plain HTTP.
- **Do not expose the switch web UI to WAN** — run on a management VLAN.

## Architecture

See [docs/DESIGN.md](docs/DESIGN.md) for the full design document, API
reference, and data model documentation.

## License

Apache 2.0
