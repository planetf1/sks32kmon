# GoodTop ZX310S-8T2XS firmware 1.0.0.6 — embedded Web UI/API notes

Extracted from `mxl862xxs_backup_image_24092026.bin` and confirmed live
against a physical ZX310S-8T2XS at firmware 1.0.0.6. Implemented in
`src/v1.rs` (wire-format parsing/building) and `src/client.rs`
(`FirmwareFamily`-branched read/write methods).

## Auth

Identical to V2: `GET /authorize?loginusr=<md5(username)>&loginpwd=<md5(password)>`,
session cookie on success, redirect to `login.html` in the response body on
failure.

## Firmware family detection

`GET /status.json` is field-compatible with V2's `SystemInfo` (`temperature`,
`sys_ipv4`, `sys_macaddr`, `fw_ver`, `hw_ver`, `des`, plus an extra `modle`
field V2 doesn't have). Family is detected from `fw_ver` starting with `"1."`.

## Port index remapping — READ THIS FIRST

The embedded UI defines a port-mapping table swapping ports 9 and 10 (the
SFP+ uplinks) between the sequential numbering used everywhere in this
tool's UI/CLI/front panel, and the `Port_N` JSON keys / POST field suffixes
the switch actually uses on the wire. Ports 1-8 are identity. Implemented as
`v1::map_port` (self-inverse).

Confirmed (via two independent firmware-JS analysis passes, both consistent)
to apply to: `port_setting_load`/`apply_user_port_setting` (outer key +
`port_list`), `port_statistics`/`clear_statistics` (outer key), `port_vlan_cfg`/
`save_port_vlan_map` (outer key + every per-port POST field), `port_mirror`
(outer key, `MonitoringPortId`, `mirrored_port_selection`,
`mirroring_port_selection`), `port_lock_cfg`/`port_loop_status` (outer key +
`psel_cbox<N>`), `stp` (outer key + `psel_cbox<N>`), `tag_vlan_cfg` (only the
`pP_N`/`ppName_N` port-id *value*, NOT the `bP_N` row index — that's a
0-based table row, not a port), `igmp_add_static_entries` (`port-input`, both
directions).

**NOT remapped** (confirmed by grep — no functional use of the mapping table
found near these): `storm_ctrl_cfg` (switch-global, no port index at all),
`eee_config` (switch-global), `mac_get_dynamic_mac_entries` (`Dynamic_portid`
passed through raw — UNCLEAR what id-space this raw value is actually in,
not verified against real traffic), `mac_get_static_mac_entries` /
`mac_add_static_mac_entries` (`Static_portid`/`port-input` also raw/unmapped
— this one's an outlier vs. its sibling static-entry endpoints on other
tables, which *do* remap).

`port_trunk_cfg`: the firmware's own JS is internally inconsistent — the
write path and the state-sync/select-population read code consistently use
the mapped id, but one Link-Up/Down display line uses the raw index. We
mirror the majority (mapped) convention; the display-only discrepancy is
unverified against hardware.

**Live-verified**: setting `--flow off`/`--status disabled` on UI port 9
correctly modifies wire `Port_10` and leaves wire `Port_9` untouched, and
vice versa. Confirms the mapping direction is correct for port settings at
least.

## ⚠️ GET/POST vocabulary mismatch (caught live, cost a switch reboot)

**`Port_Status`**: GET reports `"Enabled"`/`"Disabled"`, but the
`<select name="selectStatus">` POST options are `"Enable"`/`"Disable"` (no
trailing "d"). Sending the GET-vocabulary string is silently rejected by the
switch and it falls back to **disabling the port** — this happened live
during development (a flow-control-only change unexpectedly disabled a
port). Fixed via `v1::status_get_to_post`.

**`Spd_Duplex_Cfg`**: GET reports `"1000MbpsFull"` (no space), but the
`<select name="selectSpeedDuplex">` POST options are `"1000Mbps Full"`
(space before Full/Half). Fixed via `v1::speed_get_to_post`.

**Flow control** (`"On"`/`"Off"`) and **STP mode** (`"STP"`/`"RSTP"`) do
*not* have this mismatch — GET and POST vocabularies match exactly,
confirmed against the `<option value=...>` markup directly.

**Lesson**: don't assume a GET response's string vocabulary is safe to POST
back unmodified on this firmware — check the actual `<select>`/`<option
value=...>` markup for each field, not just the JS that reads `formObj`.

## ⚠️ Whole-table writes and the lossy round-trip incident

Several endpoints (`apply_user_port_setting`, `port_vlan_cfg`,
`port_trunk_cfg`, `stp`) don't support single-port deltas — they require
resubmitting a value for every port in one POST, matching the embedded UI's
own "apply the whole form" behavior. This tool's read-modify-write pattern
(GET current, overlay the one port being changed, POST everything) works,
**but untouched ports must be carried forward from the exact current wire
state, not reconstructed by re-encoding a decoded/canonical representation.**

This bit us for real: `port_vlan_cfg`'s wire model
(`bpEn`/`bpVid`/`untag`/`tag` booleans) was decoded into this tool's
canonical PVID+FrameType model for display, and an earlier version of
`v1::build_port_vlan_body` reconstructed untouched ports by re-encoding
*that* canonical value instead of copying the raw wire fields. The decode
`(untag=false, tag=false) -> FrameType::All` and re-encode
`FrameType::All -> (untag=true, tag=true)` are not inverses. Since every
port was at the `(false, false)` factory default, changing VLAN settings on
one port silently rewrote the tag/untag flags on **all ten**, including
whichever port carried management traffic, and the switch dropped off the
network on every port (physical link stayed up) until power-cycled. The
`save_port_vlan_map` call issued right after happened to time out, so the
bad state was never persisted to flash and a power cycle reverted to the
last-good saved config — that's luck, not a safety mechanism to rely on.

Fixed: `v1::build_port_vlan_body` now takes the raw `GET` response `Value`
and copies untouched ports' `bpEn`/`bpVid`/`untag`/`tag` fields verbatim;
only the one port actually being changed goes through the
FrameType<->flags mapping. Covered by
`test_build_port_vlan_body_preserves_untouched_ports_verbatim`. Live-verified
via direct curl (apply-only, no save) that a port-9 PVID change touches only
wire `Port_10` and leaves all other ports byte-for-byte identical, then via
the full CLI path including the save step.

**Any future whole-table-write endpoint on this firmware should follow the
same pattern**: pass through the raw current wire object for untouched
entries, never a decoded/re-encoded canonical value.

## Endpoint reference

### Port settings
- GET `/port_setting_load.json`: `{PortNum: str, Port_<wire>: {Port_Id, Port_Status, Spd_Duplex_Cfg, Spd_Duplex_Actual, Flow_Ctrl_Cfg, Flow_Ctrl_Actual}}`. No `PortMode`, no `EEE_Status`.
- POST `/apply_user_port_setting.json`: `{port_sts, port_spd_duplex, flow_ctrl, port_num: int, port_list: [str]}` — one status/speed/flow triple applied to a list of (wire) ports, not per-port. See vocabulary mismatch above.
- POST `/save_user_port_setting.json`: `{}`.

### Port statistics
- GET `/port_statistics.json`: same shape as V2's `PortStats` per port. `clear_statistics.json` is GET, not POST (matches V2's existing implementation already).

### MAC tables
- GET `/mac_get_dynamic_mac_entries.json`, `/mac_get_next_dynamic_mac_entries.json` (no params, unclear pagination cursor), `/mac_search_dynamic_mac_entries.json?mac_search_txt=...`: `{total_entries: str, Idx_0..Idx_N: {Dynamic_idx, Dynamic_mac_addr, Dynamic_portid, Dynamic_fid, Dynamic_age_timer}}`. No VLAN id field — defaulted to 0.
- POST `/mac_clear_dynamic_mac_entries.json`: `{}`.
- GET `/mac_get_static_mac_entries.json`: `{total_entries, Idx_0..Idx_N: {Static_mac_addr, Static_portid, Static_fid}}`. No `Static_idx` (synthesized from position) or VLAN id.
- Static MAC write endpoints (`mac_add/clear/save/delete_static_mac_entries.json`) documented but not implemented — no CLI surface for static MAC writes on V2 either.

### EEE
- `/eee_config.json` returns HTTP 400 on this specific device/firmware build — not wired into this model's UI despite the JS referencing it elsewhere in the bundle. Not implemented; per-port EEE status left blank.

### Network settings
- GET `/network_settings.json`: `{ipAddress, netmask, gateway, dhcpEnabled, ipv6Address, prefixLength, ll_ipv6Address, ll_prefixLength, ipv6Gateway, dhcpEnabledIPv6}`. No DNS server field at all.
- POST `/network_settings_ipv4.json`: `{input_ip, input_netmask, input_gateway, dhcp_enable}`. Success = `{"alert_msg":"Success"}`, NOT the empty-body convention used elsewhere.
- IPv6 write (`network_settings_ipv6.json`) and SNTP (`sntp_setting.json`, 400s on this device) not implemented.

### IGMP
- GET/POST `/igmp_config.json`: `{igmp, fast_leave, snoop_querier}` (GET vocab) vs. checkbox POST `{igmp?, "fast-leave"?, "snoop-querier-sel"?}` (hyphenated names, omitted-if-unchecked). No `report_flood` — that's a V2-only concept; `snoop_querier` added as an additive `Option` field on the canonical `IgmpConfig`.
- Static IGMP entry endpoints documented but not implemented (no CLI surface on V2 either).

### Storm control
- GET/POST `/storm_ctrl_cfg.json`: `{sctrl_state: "0"|"1", sctrl_rate: str}` — **switch-global**, one rate for the whole switch, no per-port/per-traffic-type control like V2. Read approximated into the per-port canonical shape for display; write refused (no faithful translation of the CLI's per-port multi-rate request).

### Port mirror
- GET `/port_mirror.json`: per-port shape identical to V2's `PortMirrorEntry`.
- POST: `{Ingress_Status: "0"|"1", Egress_Status: "0"|"1", mirrored_port_selection: [str], mirroring_port_selection: str}` — **one shared source-port set for both directions**, not independent per-port ingress/egress like V2. Only symmetric requests (same ports for `--source-ingress`/`--source-egress`) are supported; asymmetric ones are refused.

### Trunk / LACP
- GET `/port_trunk_cfg.json` matches V2's `TrunkConfigResponse` field-for-field (real JSON ints, flattened per-port raw) — no V1-specific read parsing needed.
- POST: whole-table resubmit, `{system_priority, portTypeId_<wire>, lacpTimeoutId_<wire>, Port_<wire>_grpInd}` per port. Option values confirmed identical to GET's integer encoding (0/1/2 etc).

### Loop status (read-only)
- GET `/port_loop_status.json`: flat `Violdetd_<wire>` fields, matches canonical `LoopStatusResponse` shape exactly modulo the port mapping.
- `port_lock_cfg.json` is a *different* feature (per-port security locking) despite the firmware's own confusing "loop detection" page title — no equivalent of V2's detect_enable/time_interval/recover_time config exists on V1. `set_loop_protection` refused for V1.

### STP
- GET/POST `/stp.json`: `stp_mode` ("STP"/"RSTP") instead of V2's `stp_rstp_mode`. Per-port `Stp_Edge_N` values observed live were NOT simple booleans (looked bit-shifted, e.g. port 3 -> "8" = 2^3) — passed through into `raw` verbatim rather than decoded, since the naive "0=not edge" JS-inferred heuristic doesn't match what real hardware returned. `stp_enable` write is explicitly translated by the firmware's own JS from checkbox-checked to `"1"`/`"0"` strings (not the checkbox-omission convention) — confirmed both live and in source.

### Backup/restore
- **Not implemented.** V1's native config backup/restore is a custom binary key=value blob (confirmed from a real downloaded backup: `sys/stp/stp_mode=STP`, `sys/ip/ipv4_gw=...`, `sys/port/setting_data=<binary>`, etc.), not JSON. `backup_config`/`restore_config` return a clear error for V1 pointing users at the switch's own web UI.

### Misc / not implemented
`tag_vlan_cfg`/`save_tag_vlan_map` (802.1Q tagged VLAN table, 111 rows on
this device — no corresponding V2 feature or CLI surface), `user_ac_cfg`
(password change, POSTs MD5 hashes of username/current/new password — no
CLI surface), `language_info` (POST-only in practice, GET is dead/commented
out in this firmware build), `init_vlan` (dead code, commented out).

## Status

All of the above is implemented in `src/v1.rs` + `src/client.rs`, covered by
unit tests with fixtures captured from real hardware, and live-tested
end-to-end against a physical ZX310S-8T2XS (firmware 1.0.0.6) for: status,
network settings, port settings (read + write, including the vocabulary
fix), port statistics, dynamic MAC, static MAC, trunk, VLAN (read + write,
including the whole-table-preservation fix), STP (read + enable/disable
write), loop status, IGMP (read + write), storm control (read), port mirror
(read). Not live-tested: port mirror write, trunk write, static MAC/IGMP
static-entry writes (not implemented), EEE (not implemented, 400s on this
device), tagged VLAN (not implemented).
