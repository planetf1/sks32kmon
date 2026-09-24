//! V1 firmware family — GoodTop ZX310S-8T2XS, firmware 1.0.0.6 (and presumably
//! other MaxLinear MXL862xx-based switches sharing this web UI, e.g. XikeStor
//! switches on older firmware — detected by `fw_ver` starting with `"1."`).
//!
//! This is the *same physical hardware family* as the V2-targeted SKS3200-8E2X
//! (10 ports: 8 copper + 2 SFP+), but an earlier/different-branded firmware
//! build with different endpoint names, JSON field names, and in a few cases a
//! genuinely different data model (EEE and storm control are switch-global
//! here rather than per-port; VLAN membership is a bridge-port + 802.1Q table
//! rather than a flat PVID/frame-type model).
//!
//! These functions convert between the V1 wire format and the canonical types
//! in `models.rs` / `write_models.rs`, so the rest of the app (CLI, TUI, mock)
//! stays completely unaware of firmware family — `client.rs` is the only other
//! file that knows this module exists.
//!
//! Sourced from the embedded web UI JS in the switch's own firmware image
//! (see `V1_API_NOTES.md`) and cross-checked live against a real
//! ZX310S-8T2XS at firmware 1.0.0.6.
//!
//! ## Port index remapping
//!
//! The V1 embedded UI defines a port-mapping table that swaps the sequential
//! port numbering used everywhere in this tool's UI/CLI (and printed on the
//! front panel) with the `Port_N` JSON keys / POST field values the switch
//! actually uses on the wire: ports 1-8 are unchanged, but UI port 9 <-> wire
//! port 10, and UI port 10 <-> wire port 9. This affects most, but not all,
//! endpoints — see [`map_port`] and the per-endpoint notes below. It was
//! confirmed by two independent passes over the firmware JS and is consistent
//! across every endpoint it applies to, but has *not* been physically
//! verified against a transceiver in SFP+ slot 9/10 (both were unpopulated
//! during development) — worth a sanity check before relying on it.

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::Value;

use crate::models::*;
use crate::write_models::*;

// ---------------------------------------------------------------------------
// Firmware family detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwareFamily {
    /// GoodTop ZX310S-8T2XS and similar, fw_ver starting "1."
    V1,
    /// XikeStor SKS3200-8E2X (the originally-supported switch), fw_ver starting "2."
    V2,
}

impl FirmwareFamily {
    /// Detect the firmware family from `status.json`'s `fw_ver` field.
    /// Defaults to V2 (the tool's original target) for anything not
    /// recognizably V1, so existing SKS3200 behaviour never changes.
    pub fn detect(fw_ver: &str) -> Self {
        if fw_ver.trim().starts_with('1') {
            FirmwareFamily::V1
        } else {
            FirmwareFamily::V2
        }
    }
}

// ---------------------------------------------------------------------------
// Port index mapping
// ---------------------------------------------------------------------------

/// UI-sequential port number <-> wire/physical port id. Self-inverse: ports
/// 1-8 are identity, 9 and 10 (the SFP+ uplinks) are swapped.
pub fn map_port(port: u32) -> u32 {
    match port {
        9 => 10,
        10 => 9,
        n => n,
    }
}

/// Fetch `Port_<wire id>` from a JSON object for the given UI-sequential port.
fn port_obj(root: &Value, ui_port: u32) -> Result<&Value> {
    let key = format!("Port_{}", map_port(ui_port));
    root.get(&key)
        .with_context(|| format!("missing {} in response", key))
}

fn as_str_field(v: &Value, field: &str) -> Result<String> {
    v.get(field)
        .with_context(|| format!("missing field {}", field))?
        .as_str()
        .map(|s| s.to_string())
        .with_context(|| format!("field {} is not a string", field))
}

// ---------------------------------------------------------------------------
// Device description — POST /set_des.json
// ---------------------------------------------------------------------------

/// V1 uses `input_des` instead of V2's `des`. GET /status.json is identical
/// between families (`des`, `fw_ver`, `hw_ver`, `sys_macaddr`, `sys_ipv4`,
/// `temperature` all match) so no read-side conversion is needed.
pub fn set_description_body(description: &str) -> Value {
    serde_json::json!({ "input_des": description })
}

// ---------------------------------------------------------------------------
// Port settings — GET /port_setting_load.json, POST /apply_user_port_setting.json
// ---------------------------------------------------------------------------
//
// V1 per-port fields lack `EEE_Status` (EEE is switch-global on this
// firmware, see below, and in practice not wired into this model's UI at
// all — GET /eee_config.json 400s on real hardware) and there's no
// `PortMode` at the top level. Port index is mapped (outer `Port_<id>` key).

#[derive(Debug, Deserialize)]
struct V1PortCfg {
    #[serde(rename = "Port_Status")]
    port_status: String,
    #[serde(rename = "Spd_Duplex_Cfg")]
    spd_duplex_cfg: String,
    #[serde(rename = "Spd_Duplex_Actual")]
    spd_duplex_actual: String,
    #[serde(rename = "Flow_Ctrl_Cfg")]
    flow_ctrl_cfg: String,
    #[serde(rename = "Flow_Ctrl_Actual")]
    flow_ctrl_actual: String,
}

pub fn parse_port_settings(raw: &Value) -> Result<PortSettingsResponse> {
    let port_num = as_str_field(raw, "PortNum").unwrap_or_else(|_| "10".to_string());
    let mut ports: Vec<PortCfg> = Vec::with_capacity(10);
    for ui_port in 1..=10u32 {
        let obj = port_obj(raw, ui_port)?;
        let v1: V1PortCfg = serde_json::from_value(obj.clone())
            .with_context(|| format!("parsing port_setting_load Port for UI port {}", ui_port))?;
        ports.push(PortCfg {
            // EEE is not exposed per-port by this firmware's UI; left blank
            // rather than asserting a state we can't confirm.
            eee_status: String::new(),
            port_id: ui_port.to_string(),
            port_status: v1.port_status,
            spd_duplex_cfg: v1.spd_duplex_cfg,
            spd_duplex_actual: v1.spd_duplex_actual,
            flow_ctrl_cfg: v1.flow_ctrl_cfg,
            flow_ctrl_actual: v1.flow_ctrl_actual,
        });
    }
    Ok(PortSettingsResponse {
        port_num,
        // Not present in V1; nothing reads this field on the V1 path.
        port_mode: String::new(),
        port_1: ports[0].clone(),
        port_2: ports[1].clone(),
        port_3: ports[2].clone(),
        port_4: ports[3].clone(),
        port_5: ports[4].clone(),
        port_6: ports[5].clone(),
        port_7: ports[6].clone(),
        port_8: ports[7].clone(),
        port_9: ports[8].clone(),
        port_10: ports[9].clone(),
    })
}

/// Build the V1 `apply_user_port_setting.json` body.
///
/// V1's apply endpoint is flat (one status/speed/flow triple applied to a
/// list of ports), unlike V2's per-port nested object. The canonical
/// `PortSettingsApplyRequest` as built by this tool's CLI always targets
/// exactly one port (`PortSettingsApplyRequest::single_port`); we find that
/// port and fill in any fields the caller left unset (`None`) from the
/// port's *current* settings, since V1 requires all three values on every
/// apply (there's no way to say "leave flow control as-is").
pub fn build_port_settings_apply_body(
    current: &PortSettingsResponse,
    req: &PortSettingsApplyRequest,
) -> Result<Value> {
    let slots: [(u32, &Option<PortSettingsRequest>, &PortCfg); 10] = [
        (1, &req.port_1, &current.port_1),
        (2, &req.port_2, &current.port_2),
        (3, &req.port_3, &current.port_3),
        (4, &req.port_4, &current.port_4),
        (5, &req.port_5, &current.port_5),
        (6, &req.port_6, &current.port_6),
        (7, &req.port_7, &current.port_7),
        (8, &req.port_8, &current.port_8),
        (9, &req.port_9, &current.port_9),
        (10, &req.port_10, &current.port_10),
    ];

    let (ui_port, settings, current_cfg) = slots
        .into_iter()
        .find_map(|(p, s, c)| s.as_ref().map(|s| (p, s, c)))
        .context("port settings request targets no port")?;

    let port_sts = settings
        .port_status
        .clone()
        .unwrap_or_else(|| current_cfg.port_status.clone());
    let port_spd_duplex = settings
        .spd_duplex_cfg
        .clone()
        .unwrap_or_else(|| current_cfg.spd_duplex_cfg.clone());
    let flow_ctrl = settings
        .flow_ctrl_cfg
        .clone()
        .unwrap_or_else(|| current_cfg.flow_ctrl_cfg.clone());

    Ok(serde_json::json!({
        "port_sts": status_get_to_post(&port_sts),
        "port_spd_duplex": speed_get_to_post(&port_spd_duplex),
        "flow_ctrl": flow_ctrl,
        "port_num": 1,
        "port_list": [map_port(ui_port).to_string()],
    }))
}

/// Translate `Port_Status`'s GET vocabulary ("Enabled"/"Disabled") to the
/// `<select name="selectStatus">` POST vocabulary ("Enable"/"Disable").
/// **This mismatch is real and was caught live**: sending "Enabled" is
/// silently rejected by the switch, which falls back to disabling the
/// port — confirmed against a physical ZX310S-8T2XS.
fn status_get_to_post(s: &str) -> String {
    match s {
        "Enabled" => "Enable".to_string(),
        "Disabled" => "Disable".to_string(),
        other => other.to_string(),
    }
}

/// Translate `Spd_Duplex_Cfg`'s GET vocabulary ("1000MbpsFull", no space)
/// to the `<select name="selectSpeedDuplex">` POST vocabulary
/// ("1000Mbps Full", space before Full/Half). "Auto" is unchanged. Also
/// caught live: the space is significant, the switch doesn't normalize it.
fn speed_get_to_post(s: &str) -> String {
    if let Some(rest) = s.strip_suffix("Full") {
        if !rest.ends_with(' ') {
            return format!("{} Full", rest);
        }
    } else if let Some(rest) = s.strip_suffix("Half") {
        if !rest.ends_with(' ') {
            return format!("{} Half", rest);
        }
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Port statistics — GET /port_statistics.json, GET /clear_statistics.json
// ---------------------------------------------------------------------------
//
// Per-port field names are identical to V2's `PortStats` (`Port_Id`,
// `Port_Status`, `Link_Status`, `Tx/RxGoodPkt`, `Tx/RxBadPkt`), so we reuse
// that type directly — only the outer port-index mapping differs.

pub fn parse_port_statistics(raw: &Value) -> Result<PortStatisticsResponse> {
    let port_num = as_str_field(raw, "PortNum").unwrap_or_else(|_| "10".to_string());
    let mut ports: Vec<PortStats> = Vec::with_capacity(10);
    for ui_port in 1..=10u32 {
        let obj = port_obj(raw, ui_port)?;
        let mut stats: PortStats = serde_json::from_value(obj.clone())
            .with_context(|| format!("parsing port_statistics Port for UI port {}", ui_port))?;
        // The wire object's own Port_Id reflects the *wire* port (e.g. UI
        // port 9 reads wire Port_10, whose embedded Port_Id is "10") — the
        // CLI labels rows from this field, so it must be overridden to the
        // UI-sequential number, not left as-is.
        stats.port_id = ui_port.to_string();
        ports.push(stats);
    }
    Ok(PortStatisticsResponse {
        port_num,
        port_1: ports[0].clone(),
        port_2: ports[1].clone(),
        port_3: ports[2].clone(),
        port_4: ports[3].clone(),
        port_5: ports[4].clone(),
        port_6: ports[5].clone(),
        port_7: ports[6].clone(),
        port_8: ports[7].clone(),
        port_9: ports[8].clone(),
        port_10: ports[9].clone(),
    })
}

// ---------------------------------------------------------------------------
// MAC tables — GET /mac_get_dynamic_mac_entries.json, /mac_get_static_mac_entries.json
// ---------------------------------------------------------------------------
//
// Different envelope from V2 (`{total_entries, Idx_0..Idx_N}` object instead
// of newline-delimited `data: [...]` lines), and no `*_vlan_id` field at all
// — defaulted to 0 rather than guessing. Port-id fields are NOT run through
// `map_port`: the firmware JS never transforms them (confirmed by grep —
// only an unused `portMappingRelationship.js` include appears near these
// endpoints), so whatever the switch returns is passed through unchanged.
// That raw value's meaning (UI-sequential vs. some other internal port-id
// space) is unconfirmed — verify against real traffic before trusting it.

#[derive(Debug, Deserialize)]
struct V1DynamicMacEntry {
    #[serde(rename = "Dynamic_idx")]
    idx: String,
    #[serde(rename = "Dynamic_mac_addr")]
    mac_addr: String,
    #[serde(rename = "Dynamic_portid")]
    port_id: String,
    #[serde(rename = "Dynamic_age_timer")]
    age_timer: String,
}

pub fn parse_dynamic_mac_entries(raw: &Value) -> Result<Vec<MacEntry>> {
    let total: usize = raw
        .get("total_entries")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or(v.as_u64().map(|n| n as usize))
        })
        .unwrap_or(0);
    let mut entries = Vec::with_capacity(total);
    for i in 0..total {
        let key = format!("Idx_{}", i);
        let Some(obj) = raw.get(&key) else { continue };
        let v1: V1DynamicMacEntry = serde_json::from_value(obj.clone())
            .with_context(|| format!("parsing dynamic MAC entry {}", key))?;
        entries.push(MacEntry {
            idx: v1.idx.parse().unwrap_or(0),
            mac_addr: v1.mac_addr,
            // V1 doesn't report a per-entry VLAN id; not present in this
            // firmware's response, so left at 0 rather than guessed.
            vlan_id: 0,
            fid: 0,
            port_id: v1.port_id.parse().unwrap_or(0),
            age_timer: v1.age_timer.parse().unwrap_or(0),
        });
    }
    Ok(entries)
}

#[derive(Debug, Deserialize)]
struct V1StaticMacEntry {
    #[serde(rename = "Static_mac_addr")]
    mac_addr: String,
    #[serde(rename = "Static_portid")]
    port_id: String,
}

pub fn parse_static_mac_entries(raw: &Value) -> Result<Vec<StaticMacEntry>> {
    let total: usize = raw
        .get("total_entries")
        .and_then(|v| {
            v.as_str()
                .and_then(|s| s.parse().ok())
                .or(v.as_u64().map(|n| n as usize))
        })
        .unwrap_or(0);
    let mut entries = Vec::with_capacity(total);
    for i in 0..total {
        let key = format!("Idx_{}", i);
        let Some(obj) = raw.get(&key) else { continue };
        let v1: V1StaticMacEntry = serde_json::from_value(obj.clone())
            .with_context(|| format!("parsing static MAC entry {}", key))?;
        entries.push(StaticMacEntry {
            // No Static_idx field on V1; synthesize from table position.
            idx: (i + 1) as u32,
            mac_addr: v1.mac_addr,
            vlan_id: 0,
            port_id: v1.port_id.parse().unwrap_or(0),
        });
    }
    Ok(entries)
}

// ---------------------------------------------------------------------------
// Network settings — GET /network_settings.json, POST /network_settings_ipv4.json
// ---------------------------------------------------------------------------
//
// V1's GET response has no `dnsServer`/`autoDnsEnabled` fields at all (DNS
// isn't configurable through this API on this firmware) — those are left as
// empty strings on read, and `set_network_settings` rejects a request that
// tries to set them rather than silently dropping the change.

pub fn parse_network_settings(raw: &Value) -> Result<NetworkSettings> {
    Ok(NetworkSettings {
        ip_address: as_str_field(raw, "ipAddress")?,
        netmask: as_str_field(raw, "netmask")?,
        gateway: as_str_field(raw, "gateway")?,
        dhcp_enabled: as_str_field(raw, "dhcpEnabled")?,
        dns_server: String::new(),
        auto_dns_enabled: String::new(),
    })
}

pub fn build_network_settings_body(
    current: &NetworkSettings,
    req: &NetworkSettingsRequest,
) -> Result<Value> {
    if req.dns_server.is_some() || req.auto_dns_enabled.is_some() {
        bail!(
            "This switch's firmware (V1/GoodTop) does not expose DNS server \
             configuration via its IPv4 settings API — the field doesn't exist \
             in its response. Omit --dns."
        );
    }
    Ok(serde_json::json!({
        "input_ip": req.ip_address.clone().unwrap_or_else(|| current.ip_address.clone()),
        "input_netmask": req.netmask.clone().unwrap_or_else(|| current.netmask.clone()),
        "input_gateway": req.gateway.clone().unwrap_or_else(|| current.gateway.clone()),
        "dhcp_enable": req.dhcp_enabled.clone().unwrap_or_else(|| current.dhcp_enabled.clone()),
    }))
}

/// V1's network-settings write uses a different success convention than
/// almost every other endpoint (`{"alert_msg":"Success"}` instead of an
/// empty body / `{"result":"success"}`).
pub fn network_settings_response_is_success(body: &str) -> bool {
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| {
            v.get("alert_msg")
                .and_then(|m| m.as_str())
                .map(|s| s == "Success")
        })
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// IGMP snooping — GET/POST /igmp_config.json
// ---------------------------------------------------------------------------
//
// GET field names (`igmp`, `fast_leave`, `snoop_querier`) differ from the
// POST checkbox names (`igmp`, `fast-leave`, `snoop-querier-sel`), and V1 has
// no `report_flood` concept at all — it has `snoop_querier` instead, which is
// a different feature (IGMP querier snooping, not flood-on-report). Rather
// than conflating the two, `IgmpConfig` gained an additive `snoop_querier`
// field alongside the existing (V2-only-meaningful) `report_flood`.
//
// POST is checkbox-based: an unchecked box is *omitted* from the body
// entirely (not sent as "off"). Since this is a whole-form resubmit of all
// three toggles at once, turning one thing on/off without touching the
// others requires reading the current state first and re-deriving which
// keys to include.

pub fn parse_igmp_config(raw: &Value) -> Result<IgmpConfig> {
    Ok(IgmpConfig {
        igmp: as_str_field(raw, "igmp")?,
        fast_leave: as_str_field(raw, "fast_leave")?,
        report_flood: String::new(),
        snoop_querier: Some(as_str_field(raw, "snoop_querier")?),
    })
}

pub fn build_igmp_config_body(current: &IgmpConfig, req: &IgmpConfigRequest) -> Value {
    let igmp_on = req.igmp.as_deref().unwrap_or(&current.igmp) == "on";
    let fast_leave_on = req.fast_leave.as_deref().unwrap_or(&current.fast_leave) == "on";
    let snoop_querier_on = req
        .report_flood
        .as_deref()
        .or(current.snoop_querier.as_deref())
        .unwrap_or("off")
        == "on";

    let mut body = serde_json::Map::new();
    if igmp_on {
        body.insert("igmp".to_string(), Value::String("on".to_string()));
    }
    if fast_leave_on {
        body.insert("fast-leave".to_string(), Value::String("on".to_string()));
    }
    if snoop_querier_on {
        body.insert(
            "snoop-querier-sel".to_string(),
            Value::String("on".to_string()),
        );
    }
    Value::Object(body)
}

// ---------------------------------------------------------------------------
// Storm control — GET/POST /storm_ctrl_cfg.json
// ---------------------------------------------------------------------------
//
// Switch-global on this firmware (one enable flag + one rate for the whole
// switch), not per-port with separate broadcast/multicast/unicast/unknown-
// multicast rates like V2. Read is approximated into the per-port canonical
// shape (same rate copied into every port's broadcast field, other rate
// fields left 0) for display; write is refused rather than guessing which
// port/rate-type the caller meant.

pub fn parse_storm_control(raw: &Value) -> Result<StormControlResponse> {
    let state = as_str_field(raw, "sctrl_state")?;
    let rate: u32 = as_str_field(raw, "sctrl_rate")?.parse().unwrap_or(0);
    let rate = if state == "1" { rate } else { 0 };
    let ports = (1..=10u32)
        .map(|port_id| StormControlPort {
            port_id,
            sctrl_bcast: rate,
            sctrl_mcast: 0,
            sctrl_unucast: 0,
            sctrl_unmcast: 0,
        })
        .collect();
    Ok(StormControlResponse { portnum: 10, ports })
}

// ---------------------------------------------------------------------------
// Port mirror — GET/POST /port_mirror.json
// ---------------------------------------------------------------------------
//
// GET per-port fields match V2's `PortMirrorEntry` exactly. But V1's write
// model is NOT per-port: `Ingress_Status`/`Egress_Status` are switch-global
// on/off flags, and `mirrored_port_selection` is a single list of source
// ports used for *both* directions at once — there's no way to mirror
// different ports for ingress vs. egress. We only support the common case
// (the same port set requested for both ingress and egress); anything
// asymmetric is refused rather than silently dropping one side.

pub fn parse_port_mirror(raw: &Value) -> Result<PortMirrorResponse> {
    let port_num = as_str_field(raw, "PortNum").unwrap_or_else(|_| "10".to_string());
    let monitoring_raw = as_str_field(raw, "MonitoringPortId").unwrap_or_else(|_| "0".to_string());
    let monitoring_port_id = monitoring_raw
        .parse::<u32>()
        .map(|p| if p == 0 { 0 } else { map_port(p) })
        .unwrap_or(0)
        .to_string();

    let mut ports: Vec<PortMirrorEntry> = Vec::with_capacity(10);
    for ui_port in 1..=10u32 {
        let obj = port_obj(raw, ui_port)?;
        let mut entry: PortMirrorEntry = serde_json::from_value(obj.clone())
            .with_context(|| format!("parsing port_mirror Port for UI port {}", ui_port))?;
        // Same fix as port_statistics: override the wire-sourced Port_Id
        // with the UI-sequential number it actually belongs to.
        entry.port_id = ui_port.to_string();
        ports.push(entry);
    }
    Ok(PortMirrorResponse {
        port_num,
        monitoring_port_id,
        port_1: ports[0].clone(),
        port_2: ports[1].clone(),
        port_3: ports[2].clone(),
        port_4: ports[3].clone(),
        port_5: ports[4].clone(),
        port_6: ports[5].clone(),
        port_7: ports[6].clone(),
        port_8: ports[7].clone(),
        port_9: ports[8].clone(),
        port_10: ports[9].clone(),
    })
}

pub fn build_port_mirror_body(req: &PortMirrorRequest) -> Result<Value> {
    let slots: [(u32, &Option<PortMirrorEntry>); 10] = [
        (1, &req.port_1),
        (2, &req.port_2),
        (3, &req.port_3),
        (4, &req.port_4),
        (5, &req.port_5),
        (6, &req.port_6),
        (7, &req.port_7),
        (8, &req.port_8),
        (9, &req.port_9),
        (10, &req.port_10),
    ];

    let ingress: Vec<u32> = slots
        .iter()
        .filter(|(_, e)| e.as_ref().is_some_and(|e| e.ingress_status == "Enabled"))
        .map(|(p, _)| *p)
        .collect();
    let egress: Vec<u32> = slots
        .iter()
        .filter(|(_, e)| e.as_ref().is_some_and(|e| e.egress_status == "Enabled"))
        .map(|(p, _)| *p)
        .collect();

    if ingress != egress {
        bail!(
            "This switch's firmware (V1/GoodTop) only supports mirroring the \
             same set of ports for both ingress and egress — it has no way to \
             mirror different ports per direction. Use the same port list for \
             --source-ingress and --source-egress."
        );
    }

    let monitor_port: u32 = req
        .monitoring_port_id
        .parse()
        .context("invalid monitoring_port_id")?;

    let mirrored: Vec<String> = ingress.iter().map(|p| map_port(*p).to_string()).collect();
    let mirroring = if monitor_port == 0 {
        "0".to_string()
    } else {
        map_port(monitor_port).to_string()
    };

    Ok(serde_json::json!({
        "Ingress_Status": if ingress.is_empty() { "0" } else { "1" },
        "Egress_Status": if egress.is_empty() { "0" } else { "1" },
        "mirrored_port_selection": mirrored,
        "mirroring_port_selection": mirroring,
    }))
}

// ---------------------------------------------------------------------------
// Port VLAN — GET/POST /port_vlan_cfg.json
// ---------------------------------------------------------------------------
//
// V1's bridge-port VLAN model (`bpEn`/`bpVid`/`untag`/`tag` booleans) maps
// only approximately onto V2's flatter PVID + Frame_Type (0=All/1=Tagged/
// 2=Untagged) model — this firmware doesn't expose a "frame type" filter as
// a single tri-state value, it has independent "accept untagged" and
// "accept tagged" flags. The heuristic below (best-effort, not confirmed
// against hardware beyond the always-zero factory-default state) treats
// untag-only as Untagged, tag-only as Tagged, and anything else as All.
// Write is a whole-form resubmit of all 10 ports (unlike V2's speculative
// single-port POST), so it needs the current config to fill in the ports
// the caller isn't touching.
//
// CAUGHT LIVE AGAINST HARDWARE: an earlier version of this function filled
// in untouched ports by round-tripping their *canonical* (frame_type-decoded)
// state back through `flags_from_frame_type`, which is lossy —
// `(untag=false, tag=false)` (every port's factory-default state) decodes to
// "All" but *reencodes* to `(untag=true, tag=true)`, not back to
// `(false, false)`. Since every port on the test switch was at that default,
// changing VLAN settings on ONE port silently rewrote the tag/untag flags on
// ALL TEN, including whichever port carried management traffic, and the
// switch dropped off the network entirely (recovered only because the
// separate `save` step never completed before it happened, so a power cycle
// reverted to the last-persisted config — don't rely on that luck twice).
// Fix: untouched ports now copy their exact current wire fields verbatim
// instead of going through the canonical model at all; only the port the
// caller is actually changing goes through the frame_type<->flags mapping.

fn frame_type_from_flags(untag: bool, tag: bool) -> u32 {
    match (untag, tag) {
        (true, false) => 2, // Untagged
        (false, true) => 1, // Tagged
        _ => 0,             // All (both or neither)
    }
}

fn flags_from_frame_type(frame_type: u32) -> (bool, bool) {
    match frame_type {
        2 => (true, false),
        1 => (false, true),
        _ => (true, true),
    }
}

pub fn parse_port_vlan(raw: &Value) -> Result<PortVlanResponse> {
    let port_num: u32 = as_str_field(raw, "totBports")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(10);

    let mut ports: Vec<PortVlanEntry> = Vec::with_capacity(10);
    for ui_port in 1..=10u32 {
        let obj = port_obj(raw, ui_port)?;
        let wire = map_port(ui_port);
        let bp_vid: u32 = as_str_field(obj, &format!("bpVid_{}", wire))?
            .parse()
            .unwrap_or(0);
        let untag = as_str_field(obj, &format!("untag_{}", wire))? == "1";
        let tag = as_str_field(obj, &format!("tag_{}", wire))? == "1";
        ports.push(PortVlanEntry {
            port_id: ui_port,
            pvid: bp_vid,
            frame_type: frame_type_from_flags(untag, tag),
        });
    }

    Ok(PortVlanResponse {
        port_num,
        port_1: ports[0].clone(),
        port_2: ports[1].clone(),
        port_3: ports[2].clone(),
        port_4: ports[3].clone(),
        port_5: ports[4].clone(),
        port_6: ports[5].clone(),
        port_7: ports[6].clone(),
        port_8: ports[7].clone(),
        port_9: ports[8].clone(),
        port_10: ports[9].clone(),
    })
}

/// `current` is the *raw* wire JSON from `GET port_vlan_cfg.json` (not the
/// lossy canonical `PortVlanResponse`) — untouched ports must copy their
/// exact current `bpEn`/`bpVid`/`untag`/`tag` fields verbatim, byte for
/// byte, rather than being reconstructed from the canonical model. See the
/// module-level comment above for why: that reconstruction is lossy and
/// broke the switch's own network reachability in testing.
pub fn build_port_vlan_body(current: &Value, req: &PortVlanRequest) -> Result<Value> {
    let slots: [(u32, &Option<VlanPortEntry>); 10] = [
        (1, &req.port_1),
        (2, &req.port_2),
        (3, &req.port_3),
        (4, &req.port_4),
        (5, &req.port_5),
        (6, &req.port_6),
        (7, &req.port_7),
        (8, &req.port_8),
        (9, &req.port_9),
        (10, &req.port_10),
    ];

    let mut body = serde_json::Map::new();
    for (ui_port, requested) in slots {
        let wire = map_port(ui_port);

        let (bp_en, bp_vid, untag, tag) = match requested {
            // This port is being explicitly changed: derive its new wire
            // fields from the requested PVID/frame-type.
            Some(entry) => {
                let (untag, tag) = flags_from_frame_type(entry.frame_type);
                (entry.pvid != 0, entry.pvid.to_string(), untag, tag)
            }
            // Untouched: copy the switch's own current fields exactly,
            // with no reinterpretation through the canonical model.
            None => {
                let cur_obj = port_obj(current, ui_port)?;
                let bp_en = as_str_field(cur_obj, &format!("bpEn_{}", wire))? == "1";
                let bp_vid = as_str_field(cur_obj, &format!("bpVid_{}", wire))?;
                let untag = as_str_field(cur_obj, &format!("untag_{}", wire))? == "1";
                let tag = as_str_field(cur_obj, &format!("tag_{}", wire))? == "1";
                (bp_en, bp_vid, untag, tag)
            }
        };

        // Checkbox-omission convention (confirmed for this firmware's
        // checkbox fields generally): unchecked/false is omitted, not sent
        // as "off".
        if bp_en {
            body.insert(
                format!("checkbox_{}", wire),
                Value::String("on".to_string()),
            );
        }
        body.insert(format!("fidName_{}", wire), Value::String(bp_vid));
        if untag {
            body.insert(
                format!("checkboxUntag_{}", wire),
                Value::String("on".to_string()),
            );
        }
        if tag {
            body.insert(
                format!("checkboxTag_{}", wire),
                Value::String("on".to_string()),
            );
        }
    }
    Ok(Value::Object(body))
}

// ---------------------------------------------------------------------------
// Trunk / LACP — GET/POST /port_trunk_cfg.json
// ---------------------------------------------------------------------------
//
// GET response shape (`PortNum`/`system_priority` as real JSON ints, rest
// flattened) matches V2's `TrunkConfigResponse` field-for-field already —
// parses directly with no conversion needed. Write is a whole-form resubmit
// of all 10 ports at once (unlike V2's per-port POST), so single-port CLI
// updates need read-modify-write, same pattern as port settings/VLAN.
//
// The firmware's own JS is internally inconsistent about whether the Link
// Up/Down *display* re-reads the mapped or unmapped outer key (one line out
// of four uses the raw index) — flagged as unverified; we mirror the
// majority (mapped) convention used everywhere else in this firmware.

pub fn build_trunk_config_body(current: &Value, req: &TrunkConfigRequest) -> Result<Value> {
    let slots: [(u32, &Option<TrunkPortEntry>); 10] = [
        (1, &req.port_1),
        (2, &req.port_2),
        (3, &req.port_3),
        (4, &req.port_4),
        (5, &req.port_5),
        (6, &req.port_6),
        (7, &req.port_7),
        (8, &req.port_8),
        (9, &req.port_9),
        (10, &req.port_10),
    ];

    let mut body = serde_json::Map::new();
    body.insert(
        "system_priority".to_string(),
        Value::from(req.system_priority),
    );

    for ui_port in 1..=10u32 {
        let wire = map_port(ui_port);
        let cur = port_obj(current, ui_port).ok();

        let cur_field = |name: &str| -> Option<Value> {
            cur.and_then(|c| c.get(format!("{}_{}", name, wire)).cloned())
        };

        let requested = slots
            .iter()
            .find(|(p, _)| *p == ui_port)
            .and_then(|(_, e)| e.as_ref());

        let port_type = requested
            .and_then(|e| e.port_type)
            .map(Value::from)
            .or_else(|| cur_field("portTypeId"))
            .unwrap_or(Value::from(0));
        let lacp_timeout = requested
            .and_then(|e| e.lacp_timeout)
            .map(Value::from)
            .or_else(|| cur_field("lacpTimeoutId"))
            .unwrap_or(Value::from(0));
        let grp_ind = requested
            .and_then(|e| e.group_index)
            .map(Value::from)
            .or_else(|| cur_field("grpInd"))
            .unwrap_or(Value::from(0));

        body.insert(format!("portTypeId_{}", wire), port_type);
        body.insert(format!("lacpTimeoutId_{}", wire), lacp_timeout);
        body.insert(format!("Port_{}_grpInd", wire), grp_ind);
    }

    Ok(Value::Object(body))
}

// ---------------------------------------------------------------------------
// Loop status — GET /port_loop_status.json (read-only; no config endpoint)
// ---------------------------------------------------------------------------
//
// Flat `Violdetd_N` fields matching canonical `LoopStatusResponse` exactly;
// only the port-index mapping differs. There's no equivalent of V2's
// `detect_enable`/`time_interval`/`recover_time` config on this firmware —
// the closest thing (`port_lock_cfg.json`) is actually per-port security
// locking, a different feature, despite the confusing "loop detection" page
// title in the firmware's own UI. `set_loop_protection` is refused for V1.

pub fn parse_loop_status(raw: &Value) -> Result<LoopStatusResponse> {
    let port_num = as_str_field(raw, "PortNum").unwrap_or_else(|_| "10".to_string());
    let v = |ui_port: u32| -> Result<String> {
        as_str_field(raw, &format!("Violdetd_{}", map_port(ui_port)))
    };
    Ok(LoopStatusResponse {
        port_num,
        viol_det_1: v(1)?,
        viol_det_2: v(2)?,
        viol_det_3: v(3)?,
        viol_det_4: v(4)?,
        viol_det_5: v(5)?,
        viol_det_6: v(6)?,
        viol_det_7: v(7)?,
        viol_det_8: v(8)?,
        viol_det_9: v(9)?,
        viol_det_10: v(10)?,
    })
}

// ---------------------------------------------------------------------------
// STP — GET/POST /stp.json
// ---------------------------------------------------------------------------
//
// `stp_mode` ("STP"/"RSTP") replaces V2's `stp_rstp_mode`. Per-port fields
// (`Stp_Edge_N`, `Stp_Status_N`, `Hw_Port_Id_N`) turned out on real hardware
// to be richer than the firmware JS's simple "0 = not edge" read suggested
// (`Stp_Edge_N` looked like a bit-shifted value, e.g. port 3 -> "8" = 2^3,
// not a plain boolean) — rather than assert an interpretation we can't
// confirm, the per-port detail is passed through verbatim (remapped by
// index) into `raw` instead of being decoded.
//
// Only `stp_enable` is ever written by this tool's CLI (`set stp
// enable|disable`); the write preserves the switch's current `stp_mode` and
// never touches per-port edge flags.

pub fn parse_stp_config(raw: &Value) -> Result<StpConfig> {
    let stp_enable = as_str_field(raw, "stp_enable")?;
    let stp_mode = as_str_field(raw, "stp_mode")?;
    let num_ports = as_str_field(raw, "num_ports").unwrap_or_else(|_| "10".to_string());

    let mut per_port = serde_json::Map::new();
    for ui_port in 1..=10u32 {
        if let Ok(obj) = port_obj(raw, ui_port) {
            per_port.insert(format!("Port_{}", ui_port), obj.clone());
        }
    }

    Ok(StpConfig {
        stp_enable,
        stp_rstp_mode: stp_mode,
        num_ports,
        raw: Value::Object(per_port),
    })
}

pub fn build_stp_config_body(current_stp_mode: &str, req: &StpConfigRequest) -> Value {
    let stp_enable = req.stp_enable.clone().unwrap_or_else(|| "0".to_string());
    let stp_mode = req
        .stp_rstp_mode
        .clone()
        .unwrap_or_else(|| current_stp_mode.to_string());
    serde_json::json!({
        "stp_enable": stp_enable,
        "stp_mode": stp_mode,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn port_setting_load_fixture() -> Value {
        // Captured live from a ZX310S-8T2XS at fw 1.0.0.6.
        serde_json::json!({
            "PortNum":"10",
            "Port_1":{"Port_Id":"1","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"1000MbpsFull","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_2":{"Port_Id":"2","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_3":{"Port_Id":"3","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_4":{"Port_Id":"4","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_5":{"Port_Id":"5","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_6":{"Port_Id":"6","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_7":{"Port_Id":"7","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_8":{"Port_Id":"8","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_9":{"Port_Id":"9","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
            "Port_10":{"Port_Id":"10","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}
        })
    }

    #[test]
    fn test_detect_family() {
        assert_eq!(FirmwareFamily::detect("1.0.0.6"), FirmwareFamily::V1);
        assert_eq!(FirmwareFamily::detect("2.0.0.3"), FirmwareFamily::V2);
    }

    #[test]
    fn test_map_port_swap() {
        assert_eq!(map_port(9), 10);
        assert_eq!(map_port(10), 9);
        for p in 1..=8 {
            assert_eq!(map_port(p), p);
        }
        // Self-inverse.
        for p in 1..=10 {
            assert_eq!(map_port(map_port(p)), p);
        }
    }

    #[test]
    fn test_parse_port_settings_live_fixture() {
        let raw = port_setting_load_fixture();
        let parsed = parse_port_settings(&raw).unwrap();
        assert_eq!(parsed.port_num, "10");
        assert_eq!(parsed.port_1.spd_duplex_actual, "1000MbpsFull");
        assert_eq!(parsed.port_1.port_id, "1");
        assert_eq!(parsed.port_10.port_status, "Enabled");
        assert_eq!(parsed.active_port_count(), 1);
    }

    #[test]
    fn test_parse_port_settings_port_9_10_swap() {
        // Distinguish port 9 and 10 by giving them different actual speeds
        // in the *wire* JSON, then confirm the canonical struct's port_9
        // field reflects wire Port_10 and vice versa.
        let mut raw = port_setting_load_fixture();
        raw["Port_9"]["Spd_Duplex_Actual"] = Value::String("AAAA".to_string());
        raw["Port_10"]["Spd_Duplex_Actual"] = Value::String("BBBB".to_string());
        let parsed = parse_port_settings(&raw).unwrap();
        assert_eq!(parsed.port_9.spd_duplex_actual, "BBBB");
        assert_eq!(parsed.port_10.spd_duplex_actual, "AAAA");
    }

    #[test]
    fn test_parse_port_statistics_port_id_reflects_ui_position_not_wire() {
        // Regression test: caught live against real hardware. The CLI labels
        // rows using the embedded Port_Id field, so it must match the
        // UI-sequential slot, not whatever the wire object's own Port_Id
        // happened to say (which is the *wire* port's own label).
        let mut m = serde_json::Map::new();
        for p in 1..=10u32 {
            m.insert(
                format!("Port_{}", p),
                serde_json::json!({
                    "Port_Id": p.to_string(),
                    "Port_Status": "Enabled",
                    "Link_Status": "Link Down",
                    "TxGoodPkt": "0", "TxBadPkt": "0", "RxGoodPkt": "0", "RxBadPkt": "0"
                }),
            );
        }
        let raw = serde_json::json!({"PortNum": "10"});
        let mut raw = raw;
        for (k, v) in m {
            raw[k] = v;
        }
        let parsed = parse_port_statistics(&raw).unwrap();
        assert_eq!(parsed.port_9.port_id, "9");
        assert_eq!(parsed.port_10.port_id, "10");
    }

    #[test]
    fn test_build_port_settings_apply_body_fills_current_values() {
        let raw = port_setting_load_fixture();
        let current = parse_port_settings(&raw).unwrap();
        // Only change status on UI port 9 — speed/flow should fall back to
        // current values, and port_list should carry the *wire* id (10).
        let req = PortSettingsApplyRequest::single_port(
            9,
            PortSettingsRequest {
                port_status: Some("Disabled".to_string()),
                spd_duplex_cfg: None,
                flow_ctrl_cfg: None,
            },
        )
        .unwrap();
        let body = build_port_settings_apply_body(&current, &req).unwrap();
        // Sent in POST vocabulary ("Disable", not "Disabled") — confirmed
        // live that the GET vocabulary is silently rejected here.
        assert_eq!(body["port_sts"], "Disable");
        assert_eq!(body["port_spd_duplex"], "Auto");
        assert_eq!(body["flow_ctrl"], "On");
        assert_eq!(body["port_list"], serde_json::json!(["10"]));
        assert_eq!(body["port_num"], 1);
    }

    #[test]
    fn test_status_get_to_post_vocabulary() {
        assert_eq!(status_get_to_post("Enabled"), "Enable");
        assert_eq!(status_get_to_post("Disabled"), "Disable");
    }

    #[test]
    fn test_speed_get_to_post_vocabulary() {
        assert_eq!(speed_get_to_post("Auto"), "Auto");
        assert_eq!(speed_get_to_post("100MbpsFull"), "100Mbps Full");
        assert_eq!(speed_get_to_post("1000MbpsFull"), "1000Mbps Full");
        assert_eq!(speed_get_to_post("2500MbpsFull"), "2500Mbps Full");
        assert_eq!(speed_get_to_post("10GbpsFull"), "10Gbps Full");
        // Already-spaced input passes through unchanged (idempotent).
        assert_eq!(speed_get_to_post("1000Mbps Full"), "1000Mbps Full");
    }

    #[test]
    fn test_parse_dynamic_mac_entries() {
        let raw = serde_json::json!({
            "total_entries":"2",
            "Idx_0":{"Dynamic_idx":"1","Dynamic_mac_addr":"84:E5:D8:E1:FA:EB","Dynamic_fid":"0","Dynamic_portid":"1","Dynamic_age_timer":"263"},
            "Idx_1":{"Dynamic_idx":"2","Dynamic_mac_addr":"B8:E9:37:A8:84:BE","Dynamic_fid":"0","Dynamic_portid":"1","Dynamic_age_timer":"38"}
        });
        let entries = parse_dynamic_mac_entries(&raw).unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].mac_addr, "84:E5:D8:E1:FA:EB");
        assert_eq!(entries[0].port_id, 1);
        assert_eq!(entries[0].age_timer, 263);
    }

    #[test]
    fn test_parse_network_settings_no_dns() {
        let raw = serde_json::json!({
            "ipAddress":"192.168.1.19","netmask":"255.255.255.0","gateway":"192.168.1.1","dhcpEnabled":"0",
            "ipv6Address":"2001:db8::1","prefixLength":"64"
        });
        let parsed = parse_network_settings(&raw).unwrap();
        assert_eq!(parsed.ip_address, "192.168.1.19");
        assert_eq!(parsed.dns_server, "");
    }

    #[test]
    fn test_network_settings_dns_rejected() {
        let current = NetworkSettings {
            ip_address: "192.168.1.19".into(),
            netmask: "255.255.255.0".into(),
            gateway: "192.168.1.1".into(),
            dhcp_enabled: "0".into(),
            dns_server: "".into(),
            auto_dns_enabled: "".into(),
        };
        let req = NetworkSettingsRequest {
            dns_server: Some("8.8.8.8".into()),
            ..Default::default()
        };
        assert!(build_network_settings_body(&current, &req).is_err());
    }

    #[test]
    fn test_network_settings_success_convention() {
        assert!(network_settings_response_is_success(
            r#"{"alert_msg":"Success"}"#
        ));
        assert!(!network_settings_response_is_success(
            r#"{"alert_msg":"Fail"}"#
        ));
        assert!(!network_settings_response_is_success(""));
    }

    #[test]
    fn test_parse_igmp_config_live_fixture() {
        let raw = serde_json::json!({"igmp":"off","fast_leave":"on","snoop_querier":"off"});
        let parsed = parse_igmp_config(&raw).unwrap();
        assert_eq!(parsed.igmp, "off");
        assert_eq!(parsed.fast_leave, "on");
        assert_eq!(parsed.snoop_querier.as_deref(), Some("off"));
    }

    #[test]
    fn test_build_igmp_config_body_checkbox_omission() {
        let current = IgmpConfig {
            igmp: "off".into(),
            fast_leave: "on".into(),
            report_flood: String::new(),
            snoop_querier: Some("off".into()),
        };
        // Only flip igmp on; fast_leave should stay on (carried from
        // current), snoop_querier stays omitted (off).
        let req = IgmpConfigRequest {
            igmp: Some("on".to_string()),
            fast_leave: None,
            report_flood: None,
        };
        let body = build_igmp_config_body(&current, &req);
        assert_eq!(body["igmp"], "on");
        assert_eq!(body["fast-leave"], "on");
        assert!(body.get("snoop-querier-sel").is_none());
    }

    #[test]
    fn test_parse_storm_control_disabled_zeroes_rate() {
        let raw = serde_json::json!({"sctrl_rate":"100","sctrl_state":"0"});
        let parsed = parse_storm_control(&raw).unwrap();
        assert_eq!(parsed.ports[0].sctrl_bcast, 0);
    }

    #[test]
    fn test_build_port_mirror_body_symmetric() {
        let mut req = PortMirrorRequest {
            port_num: "10".into(),
            monitoring_port_id: "9".into(),
            port_1: None,
            port_2: None,
            port_3: None,
            port_4: None,
            port_5: None,
            port_6: None,
            port_7: None,
            port_8: None,
            port_9: None,
            port_10: None,
        };
        req.port_1 = Some(PortMirrorEntry {
            port_id: "1".into(),
            ingress_status: "Enabled".into(),
            egress_status: "Enabled".into(),
        });
        let body = build_port_mirror_body(&req).unwrap();
        assert_eq!(body["Ingress_Status"], "1");
        assert_eq!(body["Egress_Status"], "1");
        assert_eq!(body["mirrored_port_selection"], serde_json::json!(["1"]));
        // Monitor port 9 (UI) -> wire 10.
        assert_eq!(body["mirroring_port_selection"], "10");
    }

    #[test]
    fn test_build_port_mirror_body_asymmetric_rejected() {
        let mut req = PortMirrorRequest {
            port_num: "10".into(),
            monitoring_port_id: "0".into(),
            port_1: None,
            port_2: None,
            port_3: None,
            port_4: None,
            port_5: None,
            port_6: None,
            port_7: None,
            port_8: None,
            port_9: None,
            port_10: None,
        };
        req.port_1 = Some(PortMirrorEntry {
            port_id: "1".into(),
            ingress_status: "Enabled".into(),
            egress_status: "Disabled".into(),
        });
        assert!(build_port_mirror_body(&req).is_err());
    }

    fn port_vlan_fixture() -> Value {
        let mut m = serde_json::Map::new();
        m.insert("totBports".to_string(), Value::String("10".to_string()));
        for p in 1..=10u32 {
            m.insert(
                format!("Port_{}", p),
                serde_json::json!({
                    format!("bpEn_{}", p): "0",
                    format!("bpVid_{}", p): "0",
                    format!("untag_{}", p): "0",
                    format!("tag_{}", p): "0",
                }),
            );
        }
        Value::Object(m)
    }

    #[test]
    fn test_parse_port_vlan_live_fixture() {
        let raw = port_vlan_fixture();
        let parsed = parse_port_vlan(&raw).unwrap();
        assert_eq!(parsed.port_num, 10);
        assert_eq!(parsed.port_1.pvid, 0);
        assert_eq!(parsed.port_1.frame_type, 0); // neither tag nor untag -> All
    }

    #[test]
    fn test_build_port_vlan_body_single_port_untagged() {
        let raw = port_vlan_fixture();
        // Set UI port 9 to PVID 20, untagged -> should write to wire port 10.
        let req = PortVlanRequest::single_port(9, 20, 2).unwrap();
        let body = build_port_vlan_body(&raw, &req).unwrap();
        assert_eq!(body["fidName_10"], "20");
        assert_eq!(body["checkboxUntag_10"], "on");
        assert!(body.get("checkboxTag_10").is_none());
        assert_eq!(body["checkbox_10"], "on");
        // Untouched port 1 should carry forward its current (disabled) state.
        assert_eq!(body["fidName_1"], "0");
        assert!(body.get("checkbox_1").is_none());
    }

    #[test]
    fn test_build_port_vlan_body_preserves_untouched_ports_verbatim() {
        // Regression test for the live incident: every port at the
        // untag=0/tag=0 factory default must round-trip back to
        // untag=0/tag=0 (both omitted) for untouched ports, NOT get
        // reconstructed as untag=on/tag=on via the canonical "All" frame
        // type. Only the one requested port should change at all.
        let raw = port_vlan_fixture();
        let req = PortVlanRequest::single_port(1, 5, 0).unwrap();
        let body = build_port_vlan_body(&raw, &req).unwrap();
        for wire in 2..=10u32 {
            assert!(
                body.get(format!("checkboxUntag_{}", wire)).is_none(),
                "port {} should not have gained checkboxUntag",
                wire
            );
            assert!(
                body.get(format!("checkboxTag_{}", wire)).is_none(),
                "port {} should not have gained checkboxTag",
                wire
            );
            assert!(
                body.get(format!("checkbox_{}", wire)).is_none(),
                "port {} should not have gained checkbox (bpEn)",
                wire
            );
            assert_eq!(body[format!("fidName_{}", wire)], "0");
        }
    }

    #[test]
    fn test_parse_loop_status_live_fixture() {
        let raw = serde_json::json!({
            "PortNum":"10","Violdetd_1":"0","Violdetd_2":"0","Violdetd_3":"0","Violdetd_4":"0","Violdetd_5":"0",
            "Violdetd_6":"0","Violdetd_7":"0","Violdetd_8":"0","Violdetd_9":"0","Violdetd_10":"0"
        });
        let parsed = parse_loop_status(&raw).unwrap();
        assert_eq!(parsed.viol_det_1, "0");
        assert_eq!(parsed.viol_det_10, "0");
    }

    #[test]
    fn test_parse_stp_config_live_fixture() {
        let raw = serde_json::json!({
            "stp_enable":"1","stp_mode":"STP","num_ports":"10",
            "Port_1":{"Stp_Edge_1":"2","Stp_Status_1":"Forward","Hw_Port_Id_1":"Port 1"},
            "Port_9":{"Stp_Edge_9":"512","Stp_Status_9":"Forward","Hw_Port_Id_9":"Port 9"},
            "Port_10":{"Stp_Edge_10":"1024","Stp_Status_10":"Forward","Hw_Port_Id_10":"Port 10"}
        });
        let parsed = parse_stp_config(&raw).unwrap();
        assert_eq!(parsed.stp_rstp_mode, "STP");
        assert_eq!(parsed.stp_enable, "1");
        // raw is keyed by UI-sequential port, sourced from the wire-mapped
        // key — but the inner field suffixes are left as the wire firmware
        // sent them (not renamed), so canonical Port_9 contains a
        // Hw_Port_Id_10 field, not Hw_Port_Id_9.
        assert_eq!(parsed.raw["Port_9"]["Hw_Port_Id_10"], "Port 10");
        assert_eq!(parsed.raw["Port_10"]["Hw_Port_Id_9"], "Port 9");
    }

    #[test]
    fn test_build_stp_config_body_preserves_mode() {
        let req = StpConfigRequest::enable();
        let body = build_stp_config_body("RSTP", &req);
        assert_eq!(body["stp_enable"], "1");
        assert_eq!(body["stp_mode"], "RSTP");
    }
}
