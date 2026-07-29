//! Write request/response types for SKS3200 configuration changes.
//!
//! These are separate from the read response models in `models.rs`
//! because the switch API uses different shapes for GET vs POST.
//! Many POST body shapes are reverse-engineered from the web UI HTML
//! and are annotated with confidence levels.

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::models::PortMirrorEntry;

// ---------------------------------------------------------------------------
// Common result wrapper — all write endpoints return this
// ---------------------------------------------------------------------------

/// Standard response from a write endpoint.
/// Real switch returns `{"result": "success"}` or `{"result": "failure"}`.
/// Mock always returns success.
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WriteResponse {
    pub result: Option<String>,
}

impl WriteResponse {
    pub fn is_success(&self) -> bool {
        self.result.as_deref() == Some("success")
    }
}

// ---------------------------------------------------------------------------
// Device description
// ---------------------------------------------------------------------------

/// Body for `POST /set_des.json`
/// **Confidence: certain** (captured from web UI)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SetDescriptionRequest {
    pub des: String,
}

// ---------------------------------------------------------------------------
// Port settings
// ---------------------------------------------------------------------------

/// Body for `POST /apply_user_port_setting.json`
/// Send only the fields you want to change.
/// **Confidence: likely** (inferred from GET response shape)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PortSettingsRequest {
    /// Admin status: "Enabled" or "Disabled"
    #[serde(rename = "Port_Status")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_status: Option<String>,

    /// Speed/duplex config: "Auto", "100MbpsFull", "1000MbpsFull", "2500MbpsFull", "10GbpsFull"
    #[serde(rename = "Spd_Duplex_Cfg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spd_duplex_cfg: Option<String>,

    /// Flow control: "On" or "Off"
    #[serde(rename = "Flow_Ctrl_Cfg")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub flow_ctrl_cfg: Option<String>,
}

/// Apply port settings to one or more ports.
/// **Confidence: likely** (inferred from GET response shape)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct PortSettingsApplyRequest {
    #[serde(rename = "PortNum")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_num: Option<String>,

    #[serde(rename = "PortMode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_mode: Option<String>,

    #[serde(rename = "Port_1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_1: Option<PortSettingsRequest>,
    #[serde(rename = "Port_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_2: Option<PortSettingsRequest>,
    #[serde(rename = "Port_3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_3: Option<PortSettingsRequest>,
    #[serde(rename = "Port_4")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_4: Option<PortSettingsRequest>,
    #[serde(rename = "Port_5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_5: Option<PortSettingsRequest>,
    #[serde(rename = "Port_6")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_6: Option<PortSettingsRequest>,
    #[serde(rename = "Port_7")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_7: Option<PortSettingsRequest>,
    #[serde(rename = "Port_8")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_8: Option<PortSettingsRequest>,
    #[serde(rename = "Port_9")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_9: Option<PortSettingsRequest>,
    #[serde(rename = "Port_10")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_10: Option<PortSettingsRequest>,
}

impl PortSettingsApplyRequest {
    /// Create a request targeting a single port.
    ///
    /// Returns an error if `port_id` is not in the valid range (1–10).
    pub fn single_port(port_id: u32, settings: PortSettingsRequest) -> Result<Self> {
        use anyhow::bail;

        if !(1..=10).contains(&port_id) {
            bail!("Invalid port number: {}. Must be 1–10.", port_id);
        }
        let port_num = Some("10".to_string());
        let port_mode = Some("PORT_MODE_8_PLUS_2".to_string());
        let mut req = Self {
            port_num,
            port_mode,
            ..Default::default()
        };
        match port_id {
            1 => req.port_1 = Some(settings),
            2 => req.port_2 = Some(settings),
            3 => req.port_3 = Some(settings),
            4 => req.port_4 = Some(settings),
            5 => req.port_5 = Some(settings),
            6 => req.port_6 = Some(settings),
            7 => req.port_7 = Some(settings),
            8 => req.port_8 = Some(settings),
            9 => req.port_9 = Some(settings),
            10 => req.port_10 = Some(settings),
            // port_id already validated to be 1-10 above
            _ => unreachable!(),
        }
        Ok(req)
    }
}

// ---------------------------------------------------------------------------
// Network settings
// ---------------------------------------------------------------------------

/// Body for `POST /network_settings_ipv4.json`
/// **Confidence: likely** (inferred from GET /network_settings.json)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct NetworkSettingsRequest {
    #[serde(rename = "ipAddress")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip_address: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub netmask: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub gateway: Option<String>,

    /// "0" = static, "1" = DHCP
    #[serde(rename = "dhcpEnabled")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dhcp_enabled: Option<String>,

    #[serde(rename = "dnsServer")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dns_server: Option<String>,

    /// "0" = manual, "1" = auto from DHCP
    #[serde(rename = "autoDnsEnabled")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_dns_enabled: Option<String>,
}

// ---------------------------------------------------------------------------
// VLAN
// ---------------------------------------------------------------------------

/// Body for `POST /port_vlan.json` (set per-port PVID and frame type)
/// **Confidence: speculative** (not yet captured from web UI)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortVlanRequest {
    #[serde(rename = "PortNum")]
    pub port_num: u32,

    #[serde(rename = "Port_1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_1: Option<VlanPortEntry>,
    #[serde(rename = "Port_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_2: Option<VlanPortEntry>,
    #[serde(rename = "Port_3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_3: Option<VlanPortEntry>,
    #[serde(rename = "Port_4")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_4: Option<VlanPortEntry>,
    #[serde(rename = "Port_5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_5: Option<VlanPortEntry>,
    #[serde(rename = "Port_6")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_6: Option<VlanPortEntry>,
    #[serde(rename = "Port_7")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_7: Option<VlanPortEntry>,
    #[serde(rename = "Port_8")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_8: Option<VlanPortEntry>,
    #[serde(rename = "Port_9")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_9: Option<VlanPortEntry>,
    #[serde(rename = "Port_10")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_10: Option<VlanPortEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct VlanPortEntry {
    #[serde(rename = "Port_Id")]
    pub port_id: u32,
    #[serde(rename = "PVID")]
    pub pvid: u32,
    /// 0 = All, 1 = Tagged, 2 = Untagged
    #[serde(rename = "Frame_Type")]
    pub frame_type: u32,
}

impl PortVlanRequest {
    /// Create a VLAN request for a single port.
    ///
    /// Returns an error if `port_id` is not in the valid range (1–10).
    pub fn single_port(port_id: u32, pvid: u32, frame_type: u32) -> Result<Self> {
        use anyhow::bail;

        if !(1..=10).contains(&port_id) {
            bail!("Invalid port number: {}. Must be 1–10.", port_id);
        }

        let entry = Some(VlanPortEntry {
            port_id,
            pvid,
            frame_type,
        });
        Ok(Self {
            port_num: 10,
            port_1: if port_id == 1 { entry.clone() } else { None },
            port_2: if port_id == 2 { entry.clone() } else { None },
            port_3: if port_id == 3 { entry.clone() } else { None },
            port_4: if port_id == 4 { entry.clone() } else { None },
            port_5: if port_id == 5 { entry.clone() } else { None },
            port_6: if port_id == 6 { entry.clone() } else { None },
            port_7: if port_id == 7 { entry.clone() } else { None },
            port_8: if port_id == 8 { entry.clone() } else { None },
            port_9: if port_id == 9 { entry.clone() } else { None },
            port_10: if port_id == 10 { entry } else { None },
        })
    }
}

// ---------------------------------------------------------------------------
// IGMP
// ---------------------------------------------------------------------------

/// Body for `POST /igmp_config.json`
/// **Confidence: speculative** (inferred from GET response)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct IgmpConfigRequest {
    /// "on" or "off"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub igmp: Option<String>,
    /// "on" or "off"
    #[serde(rename = "fast_leave")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fast_leave: Option<String>,
    /// "on" or "off"
    #[serde(rename = "report_flood")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_flood: Option<String>,
}

// ---------------------------------------------------------------------------
// Storm control
// ---------------------------------------------------------------------------

/// Body for `POST /storm_ctrl_cfg.json`
/// **Confidence: speculative** (inferred from GET response)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StormControlRequest {
    pub portnum: u32,
    pub ports: Vec<StormControlPortRequest>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StormControlPortRequest {
    pub port_id: u32,
    pub sctrl_bcast: u32,
    pub sctrl_mcast: u32,
    pub sctrl_unucast: u32,
    pub sctrl_unmcast: u32,
}

// ---------------------------------------------------------------------------
// Port mirror
// ---------------------------------------------------------------------------

/// Body for `POST /port_mirror.json`
/// **Confidence: speculative** (inferred from GET response)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortMirrorRequest {
    #[serde(rename = "PortNum")]
    pub port_num: String,

    #[serde(rename = "MonitoringPortId")]
    pub monitoring_port_id: String,

    #[serde(rename = "Port_1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_1: Option<PortMirrorEntry>,
    #[serde(rename = "Port_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_2: Option<PortMirrorEntry>,
    #[serde(rename = "Port_3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_3: Option<PortMirrorEntry>,
    #[serde(rename = "Port_4")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_4: Option<PortMirrorEntry>,
    #[serde(rename = "Port_5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_5: Option<PortMirrorEntry>,
    #[serde(rename = "Port_6")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_6: Option<PortMirrorEntry>,
    #[serde(rename = "Port_7")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_7: Option<PortMirrorEntry>,
    #[serde(rename = "Port_8")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_8: Option<PortMirrorEntry>,
    #[serde(rename = "Port_9")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_9: Option<PortMirrorEntry>,
    #[serde(rename = "Port_10")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_10: Option<PortMirrorEntry>,
}

// ---------------------------------------------------------------------------
// STP
// ---------------------------------------------------------------------------

/// Body for `POST /stp.json`
/// **Confidence: speculative** (inferred from GET response)
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct StpConfigRequest {
    /// "0" = disabled, "1" = enabled
    #[serde(rename = "stp_enable")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stp_enable: Option<String>,

    #[serde(rename = "stp_rstp_mode")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stp_rstp_mode: Option<String>,

    #[serde(rename = "num_ports")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_ports: Option<String>,

    /// Flattened per-port edge settings
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

impl StpConfigRequest {
    pub fn enable() -> Self {
        Self {
            stp_enable: Some("1".to_string()),
            ..Default::default()
        }
    }

    pub fn disable() -> Self {
        Self {
            stp_enable: Some("0".to_string()),
            ..Default::default()
        }
    }
}

// ---------------------------------------------------------------------------
// Trunk / LACP
// ---------------------------------------------------------------------------

/// Body for `POST /port_trunk_cfg.json`
/// **Confidence: speculative** (inferred from GET response)
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrunkConfigRequest {
    #[serde(rename = "PortNum")]
    pub port_num: u32,
    pub system_priority: u32,

    #[serde(rename = "Port_1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_1: Option<TrunkPortEntry>,
    #[serde(rename = "Port_2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_2: Option<TrunkPortEntry>,
    #[serde(rename = "Port_3")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_3: Option<TrunkPortEntry>,
    #[serde(rename = "Port_4")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_4: Option<TrunkPortEntry>,
    #[serde(rename = "Port_5")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_5: Option<TrunkPortEntry>,
    #[serde(rename = "Port_6")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_6: Option<TrunkPortEntry>,
    #[serde(rename = "Port_7")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_7: Option<TrunkPortEntry>,
    #[serde(rename = "Port_8")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_8: Option<TrunkPortEntry>,
    #[serde(rename = "Port_9")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_9: Option<TrunkPortEntry>,
    #[serde(rename = "Port_10")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_10: Option<TrunkPortEntry>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrunkPortEntry {
    /// NOTE: Serde renames are hardcoded to `_1` suffix. When this struct is
    /// used in `TrunkConfigRequest.port_2` through `port_10`, the field names
    /// in the JSON output are still `portTypeId_1`, `Port_1_grpInd`, etc.
    /// The real switch may misparse this for ports 2-10.
    /// FIXME: Needs real-hardware testing to determine correct wire format.
    #[serde(rename = "portTypeId_1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_type: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_priority: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lacp_timeout: Option<u32>,
    #[serde(rename = "Port_1_grpInd")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<u32>,
}

// ---------------------------------------------------------------------------
// Loop protection config
// ---------------------------------------------------------------------------

/// Body for `POST /port_lock_cfg.json`
/// **Confidence: speculative**
#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct LoopProtectionRequest {
    #[serde(rename = "PortNum")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port_num: Option<String>,

    /// "1" = enabled, "0" = disabled
    #[serde(rename = "detect_enable")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detect_enable: Option<String>,

    /// Detection interval in seconds
    #[serde(rename = "time_interval")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_interval: Option<String>,

    /// Recovery time in seconds
    #[serde(rename = "recover_time")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recover_time: Option<String>,
}

// ---------------------------------------------------------------------------
// Backup/restore (full config snapshot)
// ---------------------------------------------------------------------------

/// Complete switch configuration snapshot for backup/restore.
/// Excludes dynamic data (MAC tables, port statistics, loop violation state).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SwitchConfigSnapshot {
    pub metadata: ConfigMetadata,
    pub system_info: crate::models::SystemInfo,
    pub network_settings: crate::models::NetworkSettings,
    pub port_settings: crate::models::PortSettingsResponse,
    pub port_vlan: crate::models::PortVlanResponse,
    pub igmp_config: crate::models::IgmpConfig,
    pub storm_control: crate::models::StormControlResponse,
    pub port_mirror: crate::models::PortMirrorResponse,
    pub stp_config: crate::models::StpConfig,
    pub trunk_config: crate::models::TrunkConfigResponse,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ConfigMetadata {
    /// Tool version that created the backup
    pub tool_version: String,
    /// Switch host:port used at backup time
    pub switch_host: String,
    /// Config name from config file, or host
    pub switch_name: String,
    /// ISO 8601 timestamp
    pub timestamp: String,
}
