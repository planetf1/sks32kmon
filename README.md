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

### TUI dashboard

```bash
sks3200 -s 192.168.100.7 monitor
```

Requires the `tui` feature:

```bash
cargo install --features tui --path .
```

Keys: `q` to quit, `r` to refresh, `Tab` to switch panes, arrows to scroll.

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

```bash
cargo install --path .
```

With TUI support:

```bash
cargo install --features tui --path .
```

## Security

- Credentials are transmitted as MD5 hashes (plaintext-equivalent on the wire).
- Session cookies use `http-only` over plain HTTP.
- **Do not expose the switch web UI to WAN** — run on a management VLAN.

## Architecture

See [docs/DESIGN.md](docs/DESIGN.md) for the full design document, API
reference, and data model documentation.

## License

MIT
