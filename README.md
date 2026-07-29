# sks3200 — XikeStor SKS3200-8E2X Switch Manager

A Rust CLI + TUI tool for managing **XikeStor (兮克) SKS3200-8E2X** switches
remotely. These are 8×2.5G + 2×10G SFP+ web-managed switches with a basic
HTML dashboard — no CLI, no SSH, no SNMP.

**Phase 2: Read-write.** Query system info, port status, traffic statistics,
MAC tables, VLANs, STP, loop protection, and more — plus configure ports, VLANs,
IGMP, storm control, mirroring, trunk/LACP, loop protection, STP, network settings,
description, and manage config backups. Write operations run in **mock mode by default**
(add `--apply` to write to the switch).

---

## Quick start

```bash
# Generate a config file template and edit it
sks3200 config-init > ~/.config/sks3200/config.toml
# Then edit ~/.config/sks3200/config.toml with your switch credentials

# Once configured, query all switches at once
sks3200 show status

# Port status
sks3200 show ports

# Traffic statistics (live refresh)
sks3200 show statistics --watch

# MAC address table
sks3200 show mac

# Everything at once
sks3200 show all

# JSON output for scripting
sks3200 -j show status | jq '.temperature'
```

## Example output

All commands produce colour-coded, aligned tables with live data.

### `sks3200 show status`

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

### `sks3200 show ports`

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

### `sks3200 show statistics`

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

### `sks3200 show mac`

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

### `sks3200 show vlan`

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

### `sks3200 show stp`

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

### `sks3200 show all`

Runs all supported commands in sequence and prints every section — useful for a
quick full inventory of a switch. Output combines `status`, `ports`,
`statistics`, `mac`, `vlan`, `network`, `loop`, and `stp` sections.

### TUI dashboard (`sks3200 monitor`)

A ratatui-based real-time dashboard with auto-refresh.

```
┌─────────── Header ─────────────────────────────────────────────────────┐
│ SKS3200-8E2X @ 192.168.100.7  FW: 2.0.0.3  HW: A0  52°C  Up: 4/10    │
├─────────────── Ports (4 up) ────────────┬─────── MAC Table (37) ───────┤
│ Port Status Speed  Tx/s    Rx/s  TxErr  │ MAC Address         VLAN Port│
│ P1   Up     2.5G  123.4K  45.6K    0   │ AA:BB:CC:DD:EE:01    1    2 │
│ P2   Up     1G      890   1.2K    12   │ AA:BB:CC:DD:EE:02    1    8 │
│ ...                                    │ ...                         │
│ P8   Up     2.5G   54.5M 221.4M     0   │ AA:BB:CC:DD:EE:FF    1    0 │
│ Σ            123.4M   46.8M     12     │ churn: 3                     │
├─── Sparklines ─────────────────────────┴──────────────────────────────┤
│ P1  123.4K/ 45.6K  ▁▂▃▅▇█▆▄▂▁▁▂▃▄▅▆▇█▇▆▅▄▃▂▁                         │
│ P2    890/  1.2K   ▁▁▂▃▄▅▆▇█▇▆▅▄▃▂▂▁▁▂▃▄▅▆▇█                         │
│ P4  143.8M/  9.9M  ▁▂▂▃▃▄▅▅▆▇█▇▆▆▅▄▃▂▁▁▂▂▃▄▅▆▇██                     │
├──────────────── Footer ────────────────────────────────────────────────┤
│ [q] Quit  [r] Refresh  [+/-] 3s  [←→] main  Pane: Ports  Last: 08:34 │
└────────────────────────────────────────────────────────────────────────┘
```

The port table now includes **error rate** columns (TxErr/s, RxErr/s) with
colour-coded **error ratio** (Err%): green <0.1%, yellow 0.1–1%, red >1%.
The aggregate **Σ row** sums throughput across all ports.

**Sparklines** show 60-point traffic history per active port (bar-chart trend
over ~3 minutes at the default 3s refresh).

**MAC churn** (title counter) tracks how many entries appeared or disappeared
between refreshes. The header shows the **active port count** (Up: N/10).

Run it:
```bash
sks3200 -s 192.168.100.7 monitor
```

Keys: `q` to quit, `Tab` to cycle panes (Ports → MAC → Config), `←`/`→` to
switch between configured switches, `+`/`-` to adjust refresh rate (1–60 s),
arrows to scroll, `Enter` to edit a field in the Config pane.

## Commands

### Read Commands (`show`)

| Command | Description |
|---|---|
| `show status` | System information (temperature, IP, MAC, firmware) |
| `show ports` | Port status and settings |
| `show statistics` | Port traffic statistics (add `--watch` for live refresh) |
| `show mac` | Dynamic MAC address table |
| `show static-mac` | Static MAC address table |
| `show trunk` | Link aggregation / trunk status |
| `show vlan` | VLAN configuration |
| `show stp` | Spanning Tree Protocol status |
| `show loop` | Loop protection status |
| `show igmp` | IGMP snooping configuration |
| `show storm` | Storm control configuration |
| `show mirror` | Port mirror configuration |
| `show network` | Network settings (IP, gateway, DNS) |
| `show all` | Show all information at once |

### Write Commands (`set`)

| Command | Description |
|---|---|
| `set port` | Configure port (status, speed, flow control) |
| `set description` | Set device description |
| `set vlan` | Configure VLAN per-port (PVID, frame type) |
| `set igmp` | Configure IGMP snooping |
| `set network` | Update IPv4 network settings |
| `set storm` | Configure storm control rates |
| `set stp` | Enable/disable Spanning Tree Protocol |
| `set mirror` | Configure port mirroring |
| `set trunk` | Configure trunk/LACP per-port |
| `set loop-protection` | Configure loop protection |

### Management Commands

| Command | Description |
|---|---|
| `clear statistics` | Clear port statistics counters |
| `clear mac` | Clear dynamic MAC address table |
| `save` | Save running config to startup |
| `backup` | Backup configuration to JSON file |
| `restore` | Restore configuration from backup |
| `reboot` | Reboot the switch (requires `--yes --apply`) |
| `factory-reset` | Factory reset (requires `--yes --apply`) |
| `monitor` | TUI dashboard (live monitoring) |
| `config-init` | Generate a sample config file template |
| `help` | Print help for a given subcommand |

## Write Commands — Safety Model

All write commands (`set`, `clear`, `save`, `restore`, `reboot`, `factory-reset`)
run in **mock mode by default** — they print the request that *would* be sent
without sending it.

To actually write to the switch, add `--apply`:

```bash
# Mock (default): shows what would be sent
sks3200 set port 1 --speed 1000 --flow off

# Actually apply the change
sks3200 set port 1 --speed 1000 --flow off --apply
```

Destructive commands (`reboot`, `factory-reset`) additionally require `--yes`
to confirm:

```bash
sks3200 reboot --apply --yes
sks3200 factory-reset --apply --yes
```

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

## Limitations & Known Issues

### Not yet implemented

| Feature | Reason |
|---|---|
| **Firmware upgrade** | Requires binary upload (`POST /firmware/upgrade`). Not safe to mock — needs real hardware testing. |
| **System time config** | The `GET /systemtime_settings.json` response format has not been captured from the switch. |
| **SNMP proxy** | Planned for future release — requires SNMP agent implementation. |

### Known bugs / quirks

| Issue | Impact | Mitigation |
|---|---|---|
| **Trunk config uses hardcoded `_1` suffix** (`write_models.rs:460-469`) | Setting trunk on ports 2–10 may misconfigure port 1 on real hardware. Serde renames are fixed to `portTypeId_1` / `Port_1_grpInd`. | Use the CLI `set trunk` command, not the TUI, for trunk config. Fix requires real hardware testing. |
| **`set network --dhcp on` couples DHCP and auto-DNS** | Both `dhcpEnabled` and `autoDnsEnabled` are set from the single `--dhcp` flag. Cannot configure DHCP for IP with manual DNS (or vice versa). | Use the switch web UI for split DHCP/DNS config, or a future `--auto-dns` flag. |
| **TUI Mirror editor only sets monitor port** | Source port mirror configuration (ingress/egress per-port) is not exposed in the TUI. Setting the TUI mirror field clears all source port config on real hardware. | Use `sks3200 set mirror --source-ingress "1,3"` for full mirror config. |
| **TUI Loop Protection defaults not fetched from switch** | The TUI shows hardcoded defaults ("Off", "10", "2") rather than reading actual switch state. The GET endpoint exists but is not called. | Use `sks3200 set loop-protection` to read before editing. |
| **`cmd_all` is incomplete** | The `show all` command omits trunk, IGMP, storm, and mirror sections. | Use individual `show` commands for complete output. |

## Architecture

See [docs/DESIGN.md](docs/DESIGN.md) for the full design document, API
reference, and data model documentation.

## License

Apache 2.0
