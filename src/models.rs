use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// System Information
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct SystemInfo {
    pub temperature: String,
    pub sys_ipv4: String,
    pub sys_macaddr: String,
    pub fw_ver: String,
    pub hw_ver: String,
    pub des: String,
}

// ---------------------------------------------------------------------------
// Network Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct NetworkSettings {
    #[serde(rename = "ipAddress")]
    pub ip_address: String,
    pub netmask: String,
    pub gateway: String,
    #[serde(rename = "dhcpEnabled")]
    pub dhcp_enabled: String,
    #[serde(rename = "dnsServer")]
    pub dns_server: String,
    #[serde(rename = "autoDnsEnabled")]
    pub auto_dns_enabled: String,
}

// ---------------------------------------------------------------------------
// Port Settings
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortCfg {
    #[serde(rename = "EEE_Status")]
    pub eee_status: String,
    #[serde(rename = "Port_Id")]
    pub port_id: String,
    #[serde(rename = "Port_Status")]
    pub port_status: String,
    #[serde(rename = "Spd_Duplex_Cfg")]
    pub spd_duplex_cfg: String,
    #[serde(rename = "Spd_Duplex_Actual")]
    pub spd_duplex_actual: String,
    #[serde(rename = "Flow_Ctrl_Cfg")]
    pub flow_ctrl_cfg: String,
    #[serde(rename = "Flow_Ctrl_Actual")]
    pub flow_ctrl_actual: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortSettingsResponse {
    #[serde(rename = "PortNum")]
    pub port_num: String,
    #[serde(rename = "PortMode")]
    pub port_mode: String,
    #[serde(rename = "Port_1")]
    pub port_1: PortCfg,
    #[serde(rename = "Port_2")]
    pub port_2: PortCfg,
    #[serde(rename = "Port_3")]
    pub port_3: PortCfg,
    #[serde(rename = "Port_4")]
    pub port_4: PortCfg,
    #[serde(rename = "Port_5")]
    pub port_5: PortCfg,
    #[serde(rename = "Port_6")]
    pub port_6: PortCfg,
    #[serde(rename = "Port_7")]
    pub port_7: PortCfg,
    #[serde(rename = "Port_8")]
    pub port_8: PortCfg,
    #[serde(rename = "Port_9")]
    pub port_9: PortCfg,
    #[serde(rename = "Port_10")]
    pub port_10: PortCfg,
}

impl PortSettingsResponse {
    pub fn ports(&self) -> Vec<&PortCfg> {
        vec![
            &self.port_1,
            &self.port_2,
            &self.port_3,
            &self.port_4,
            &self.port_5,
            &self.port_6,
            &self.port_7,
            &self.port_8,
            &self.port_9,
            &self.port_10,
        ]
    }

    pub fn active_port_count(&self) -> usize {
        self.ports()
            .iter()
            .filter(|p| p.port_status == "Enabled" && p.spd_duplex_actual != "Link Down")
            .count()
    }
}

// ---------------------------------------------------------------------------
// Port Statistics
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortStats {
    #[serde(rename = "Port_Id")]
    pub port_id: String,
    #[serde(rename = "Port_Status")]
    pub port_status: String,
    #[serde(rename = "Link_Status")]
    pub link_status: String,
    #[serde(rename = "TxGoodPkt")]
    pub tx_good_pkt: String,
    #[serde(rename = "TxBadPkt")]
    pub tx_bad_pkt: String,
    #[serde(rename = "RxGoodPkt")]
    pub rx_good_pkt: String,
    #[serde(rename = "RxBadPkt")]
    pub rx_bad_pkt: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortStatisticsResponse {
    #[serde(rename = "PortNum")]
    pub port_num: String,
    #[serde(rename = "Port_1")]
    pub port_1: PortStats,
    #[serde(rename = "Port_2")]
    pub port_2: PortStats,
    #[serde(rename = "Port_3")]
    pub port_3: PortStats,
    #[serde(rename = "Port_4")]
    pub port_4: PortStats,
    #[serde(rename = "Port_5")]
    pub port_5: PortStats,
    #[serde(rename = "Port_6")]
    pub port_6: PortStats,
    #[serde(rename = "Port_7")]
    pub port_7: PortStats,
    #[serde(rename = "Port_8")]
    pub port_8: PortStats,
    #[serde(rename = "Port_9")]
    pub port_9: PortStats,
    #[serde(rename = "Port_10")]
    pub port_10: PortStats,
}

impl PortStatisticsResponse {
    pub fn ports(&self) -> Vec<&PortStats> {
        vec![
            &self.port_1,
            &self.port_2,
            &self.port_3,
            &self.port_4,
            &self.port_5,
            &self.port_6,
            &self.port_7,
            &self.port_8,
            &self.port_9,
            &self.port_10,
        ]
    }
}

// ---------------------------------------------------------------------------
// Dynamic MAC Address Table
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct MacEntry {
    #[serde(rename = "Dynamic_idx")]
    pub idx: u32,
    #[serde(rename = "Dynamic_mac_addr")]
    pub mac_addr: String,
    #[serde(rename = "Dynamic_vlan_id")]
    pub vlan_id: u32,
    #[serde(rename = "Dynamic_fid")]
    pub fid: u32,
    #[serde(rename = "Dynamic_portid")]
    pub port_id: u32,
    #[serde(rename = "Dynamic_age_timer")]
    pub age_timer: u32,
}

// ---------------------------------------------------------------------------
// Static MAC Address Table
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StaticMacEntry {
    #[serde(rename = "Static_idx")]
    pub idx: u32,
    #[serde(rename = "Static_mac_addr")]
    pub mac_addr: String,
    #[serde(rename = "Static_vlan_id")]
    pub vlan_id: u32,
    #[serde(rename = "Static_portid")]
    pub port_id: u32,
}

// ---------------------------------------------------------------------------
// Link Aggregation / Trunk Config
// ---------------------------------------------------------------------------

#[allow(dead_code)]
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortTrunkCfg {
    #[serde(rename = "portTypeId_1")]
    pub port_type: u32,
    #[serde(rename = "portPriorityId_1")]
    pub port_priority: u32,
    #[serde(rename = "lacpTimeoutId_1")]
    pub lacp_timeout: u32,
    #[serde(rename = "Port_1_grpInd")]
    pub group_index: u32,
    #[serde(rename = "Port_1_state")]
    pub state: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TrunkConfigResponse {
    #[serde(rename = "PortNum")]
    pub port_num: u32,
    pub system_priority: u32,
    // Flattened per-port fields — we handle via raw JSON for flexibility
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Loop Protection / STP
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LoopStatusResponse {
    #[serde(rename = "PortNum")]
    pub port_num: String,
    #[serde(rename = "Violdetd_1")]
    pub viol_det_1: String,
    #[serde(rename = "Violdetd_2")]
    pub viol_det_2: String,
    #[serde(rename = "Violdetd_3")]
    pub viol_det_3: String,
    #[serde(rename = "Violdetd_4")]
    pub viol_det_4: String,
    #[serde(rename = "Violdetd_5")]
    pub viol_det_5: String,
    #[serde(rename = "Violdetd_6")]
    pub viol_det_6: String,
    #[serde(rename = "Violdetd_7")]
    pub viol_det_7: String,
    #[serde(rename = "Violdetd_8")]
    pub viol_det_8: String,
    #[serde(rename = "Violdetd_9")]
    pub viol_det_9: String,
    #[serde(rename = "Violdetd_10")]
    pub viol_det_10: String,
}

impl LoopStatusResponse {
    pub fn violations(&self) -> Vec<(u32, &String)> {
        let pairs = [
            (1, &self.viol_det_1),
            (2, &self.viol_det_2),
            (3, &self.viol_det_3),
            (4, &self.viol_det_4),
            (5, &self.viol_det_5),
            (6, &self.viol_det_6),
            (7, &self.viol_det_7),
            (8, &self.viol_det_8),
            (9, &self.viol_det_9),
            (10, &self.viol_det_10),
        ];
        pairs
            .iter()
            .filter(|(_, v)| **v != "0")
            .map(|(p, v)| (*p, *v))
            .collect()
    }
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StpConfig {
    pub stp_enable: String,
    pub stp_rstp_mode: String,
    pub num_ports: String,
    #[serde(flatten)]
    pub raw: serde_json::Value,
}

// ---------------------------------------------------------------------------
// VLAN
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortVlanEntry {
    #[serde(rename = "Port_Id")]
    pub port_id: u32,
    #[serde(rename = "PVID")]
    pub pvid: u32,
    #[serde(rename = "Frame_Type")]
    pub frame_type: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortVlanResponse {
    #[serde(rename = "PortNum")]
    pub port_num: u32,
    #[serde(rename = "Port_1")]
    pub port_1: PortVlanEntry,
    #[serde(rename = "Port_2")]
    pub port_2: PortVlanEntry,
    #[serde(rename = "Port_3")]
    pub port_3: PortVlanEntry,
    #[serde(rename = "Port_4")]
    pub port_4: PortVlanEntry,
    #[serde(rename = "Port_5")]
    pub port_5: PortVlanEntry,
    #[serde(rename = "Port_6")]
    pub port_6: PortVlanEntry,
    #[serde(rename = "Port_7")]
    pub port_7: PortVlanEntry,
    #[serde(rename = "Port_8")]
    pub port_8: PortVlanEntry,
    #[serde(rename = "Port_9")]
    pub port_9: PortVlanEntry,
    #[serde(rename = "Port_10")]
    pub port_10: PortVlanEntry,
}

impl PortVlanResponse {
    pub fn ports(&self) -> Vec<&PortVlanEntry> {
        vec![
            &self.port_1,
            &self.port_2,
            &self.port_3,
            &self.port_4,
            &self.port_5,
            &self.port_6,
            &self.port_7,
            &self.port_8,
            &self.port_9,
            &self.port_10,
        ]
    }
}

// ---------------------------------------------------------------------------
// IGMP
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct IgmpConfig {
    pub igmp: String,
    pub fast_leave: String,
    pub report_flood: String,
}

// ---------------------------------------------------------------------------
// Port Mirror
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortMirrorEntry {
    #[serde(rename = "Port_Id")]
    pub port_id: String,
    #[serde(rename = "Ingress_Status")]
    pub ingress_status: String,
    #[serde(rename = "Egress_Status")]
    pub egress_status: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortMirrorResponse {
    #[serde(rename = "PortNum")]
    pub port_num: String,
    #[serde(rename = "MonitoringPortId")]
    pub monitoring_port_id: String,
    #[serde(rename = "Port_1")]
    pub port_1: PortMirrorEntry,
    #[serde(rename = "Port_2")]
    pub port_2: PortMirrorEntry,
    #[serde(rename = "Port_3")]
    pub port_3: PortMirrorEntry,
    #[serde(rename = "Port_4")]
    pub port_4: PortMirrorEntry,
    #[serde(rename = "Port_5")]
    pub port_5: PortMirrorEntry,
    #[serde(rename = "Port_6")]
    pub port_6: PortMirrorEntry,
    #[serde(rename = "Port_7")]
    pub port_7: PortMirrorEntry,
    #[serde(rename = "Port_8")]
    pub port_8: PortMirrorEntry,
    #[serde(rename = "Port_9")]
    pub port_9: PortMirrorEntry,
    #[serde(rename = "Port_10")]
    pub port_10: PortMirrorEntry,
}

// ---------------------------------------------------------------------------
// Storm Control
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StormControlPort {
    pub port_id: u32,
    pub sctrl_bcast: u32,
    pub sctrl_mcast: u32,
    pub sctrl_unucast: u32,
    pub sctrl_unmcast: u32,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct StormControlResponse {
    pub portnum: u32,
    pub ports: Vec<StormControlPort>,
}

// ---------------------------------------------------------------------------
// All Port PVIDs (compact form)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PortPvidsResponse {
    pub port_pvids: Vec<u32>,
}
