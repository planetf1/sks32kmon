//! Integration tests for SKS3200 — JSON deserialization and helper functions.
//!
//! Tests use inline JSON strings that match real switch API responses captured
//! from XikeStor SKS3200-8E2X devices.

use sks3200::client::{md5_hash, parse_mac_entries, parse_static_mac_entries};
use sks3200::models::*;

// ---------------------------------------------------------------------------
// SystemInfo
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_system_info() {
    let json = r#"{
        "temperature": "53",
        "sys_ipv4": "192.168.100.7",
        "sys_macaddr": "8C:A6:82:71:8F:B4",
        "fw_ver": "2.0.0.3",
        "hw_ver": "A0",
        "des": "SKS3200-8E2X"
    }"#;
    let info: SystemInfo = serde_json::from_str(json).expect("SystemInfo deserialization failed");
    assert_eq!(info.temperature, "53");
    assert_eq!(info.sys_ipv4, "192.168.100.7");
    assert_eq!(info.sys_macaddr, "8C:A6:82:71:8F:B4");
    assert_eq!(info.fw_ver, "2.0.0.3");
    assert_eq!(info.hw_ver, "A0");
    assert_eq!(info.des, "SKS3200-8E2X");
}

// ---------------------------------------------------------------------------
// PortSettingsResponse + PortCfg
// ---------------------------------------------------------------------------

/// Build a realistic 10‑port settings JSON.
///
/// Port_2 is the only active (linked‑up) port so we can verify
/// `active_port_count()` returns 1.
fn port_settings_json() -> String {
    let port = |id: u32, speed: &str| -> String {
        format!(
            r#""Port_{id}":{{"EEE_Status":"eee_inactive","Port_Id":"{id}","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"{speed}","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}}"#,
            id = id,
            speed = speed
        )
    };
    let ports: Vec<String> = (1..=10)
        .map(|i| {
            let speed = if i == 2 { "2500MbpsFull" } else { "Link Down" };
            port(i, speed)
        })
        .collect();
    format!(
        r#"{{"PortNum":"10","PortMode":"PORT_MODE_8_PLUS_2",{}}}"#,
        ports.join(",")
    )
}

#[test]
fn test_deserialize_port_settings() {
    let json = port_settings_json();
    let resp: PortSettingsResponse =
        serde_json::from_str(&json).expect("PortSettingsResponse deserialization failed");

    assert_eq!(resp.port_num, "10");
    assert_eq!(resp.port_mode, "PORT_MODE_8_PLUS_2");

    // Spot-check a couple of ports
    assert_eq!(resp.port_1.port_id, "1");
    assert_eq!(resp.port_1.spd_duplex_actual, "Link Down");
    assert_eq!(resp.port_2.port_id, "2");
    assert_eq!(resp.port_2.spd_duplex_actual, "2500MbpsFull");
    assert_eq!(resp.port_2.eee_status, "eee_inactive");
    assert_eq!(resp.port_2.flow_ctrl_cfg, "On");
}

#[test]
fn test_port_settings_ports_returns_10() {
    let resp: PortSettingsResponse =
        serde_json::from_str(&port_settings_json()).unwrap();
    assert_eq!(resp.ports().len(), 10);
}

#[test]
fn test_active_port_count() {
    let resp: PortSettingsResponse =
        serde_json::from_str(&port_settings_json()).unwrap();
    // Port_2 is the only one with a non-"Link Down" speed
    assert_eq!(resp.active_port_count(), 1);
}

#[test]
fn test_active_port_count_all_down() {
    // All ports "Link Down" → count = 0
    let port = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"EEE_Status":"eee_inactive","Port_Id":"{id}","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| port(i)).collect();
    let json = format!(
        r#"{{"PortNum":"10","PortMode":"PORT_MODE_8_PLUS_2",{}}}"#,
        ports.join(",")
    );
    let resp: PortSettingsResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(resp.active_port_count(), 0);
}

// ---------------------------------------------------------------------------
// PortStatisticsResponse + PortStats
// ---------------------------------------------------------------------------

fn port_stats_json() -> String {
    let port = |id: u32, link: &str, tx_good: &str, rx_good: &str, rx_bad: &str| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":"{id}","Port_Status":"Enabled","Link_Status":"{link}","TxGoodPkt":"{tx_good}","TxBadPkt":"0","RxGoodPkt":"{rx_good}","RxBadPkt":"{rx_bad}"}}"#,
            id = id,
            link = link,
            tx_good = tx_good,
            rx_good = rx_good,
            rx_bad = rx_bad
        )
    };
    let ports: Vec<String> = (1..=10)
        .map(|i| {
            if i == 2 {
                port(i, "2500MbpsFull", "78925817", "24200057", "5")
            } else {
                port(i, "Link Down", "0", "0", "0")
            }
        })
        .collect();
    format!(r#"{{"PortNum":"10",{}}}"#, ports.join(","))
}

#[test]
fn test_deserialize_port_statistics() {
    let json = port_stats_json();
    let resp: PortStatisticsResponse =
        serde_json::from_str(&json).expect("PortStatisticsResponse deserialization failed");

    assert_eq!(resp.port_num, "10");
    assert_eq!(resp.port_1.port_id, "1");
    assert_eq!(resp.port_1.link_status, "Link Down");
    assert_eq!(resp.port_2.port_id, "2");
    assert_eq!(resp.port_2.link_status, "2500MbpsFull");
    assert_eq!(resp.port_2.tx_good_pkt, "78925817");
    assert_eq!(resp.port_2.rx_good_pkt, "24200057");
    assert_eq!(resp.port_2.rx_bad_pkt, "5");
}

#[test]
fn test_port_statistics_ports_returns_10() {
    let resp: PortStatisticsResponse =
        serde_json::from_str(&port_stats_json()).unwrap();
    assert_eq!(resp.ports().len(), 10);
}

// ---------------------------------------------------------------------------
// NetworkSettings
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_network_settings() {
    let json = r#"{
        "ipAddress": "192.168.100.7",
        "netmask": "255.255.255.0",
        "gateway": "192.168.100.254",
        "dhcpEnabled": "0",
        "dnsServer": "8.8.8.8",
        "autoDnsEnabled": "1"
    }"#;
    let ns: NetworkSettings =
        serde_json::from_str(json).expect("NetworkSettings deserialization failed");

    assert_eq!(ns.ip_address, "192.168.100.7");
    assert_eq!(ns.netmask, "255.255.255.0");
    assert_eq!(ns.gateway, "192.168.100.254");
    assert_eq!(ns.dhcp_enabled, "0");
    assert_eq!(ns.dns_server, "8.8.8.8");
    assert_eq!(ns.auto_dns_enabled, "1");
}

// ---------------------------------------------------------------------------
// Dynamic MAC entries (data:-prefixed format)
// ---------------------------------------------------------------------------

#[test]
fn test_parse_mac_entries_single_line() {
    let raw = concat!(
        "data: [",
        "{\"Dynamic_idx\":1,\"Dynamic_mac_addr\":\"00:0E:58:85:04:82\",",
        "\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":8,\"Dynamic_age_timer\":244}",
        "]\n"
    );
    let entries = parse_mac_entries(raw).expect("parse_mac_entries failed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].idx, 1);
    assert_eq!(entries[0].mac_addr, "00:0E:58:85:04:82");
    assert_eq!(entries[0].vlan_id, 1);
    assert_eq!(entries[0].fid, 0);
    assert_eq!(entries[0].port_id, 8);
    assert_eq!(entries[0].age_timer, 244);
}

#[test]
fn test_parse_mac_entries_multiple_lines() {
    let raw = concat!(
        "data: [{\"Dynamic_idx\":1,\"Dynamic_mac_addr\":\"00:0E:58:85:04:82\",\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":8,\"Dynamic_age_timer\":244}]\n",
        "data: [{\"Dynamic_idx\":2,\"Dynamic_mac_addr\":\"DC:A6:32:43:C4:B0\",\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":6,\"Dynamic_age_timer\":244}]\n",
    );
    let entries = parse_mac_entries(raw).expect("parse_mac_entries failed");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].mac_addr, "00:0E:58:85:04:82");
    assert_eq!(entries[1].mac_addr, "DC:A6:32:43:C4:B0");
    assert_eq!(entries[1].port_id, 6);
}

#[test]
fn test_parse_mac_entries_empty() {
    let entries = parse_mac_entries("").expect("parse_mac_entries on empty string failed");
    assert!(entries.is_empty());
}

#[test]
fn test_parse_mac_entries_no_data_prefix() {
    let entries = parse_mac_entries("some random text\nwithout prefix\n")
        .expect("parse_mac_entries on non-data text failed");
    assert!(entries.is_empty());
}

#[test]
fn test_parse_mac_entries_malformed_json_returns_err() {
    let raw = "data: {invalid json}\n";
    assert!(parse_mac_entries(raw).is_err());
}

// ---------------------------------------------------------------------------
// Static MAC entries
// ---------------------------------------------------------------------------

#[test]
fn test_parse_static_mac_entries() {
    let raw = concat!(
        "data: [{\"Static_idx\":1,\"Static_mac_addr\":\"00:11:22:33:44:55\",",
        "\"Static_vlan_id\":100,\"Static_portid\":3}]",
        "\n"
    );
    let entries = parse_static_mac_entries(raw).expect("parse_static_mac_entries failed");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].idx, 1);
    assert_eq!(entries[0].mac_addr, "00:11:22:33:44:55");
    assert_eq!(entries[0].vlan_id, 100);
    assert_eq!(entries[0].port_id, 3);
}

#[test]
fn test_parse_static_mac_entries_empty() {
    let entries =
        parse_static_mac_entries("").expect("parse_static_mac_entries on empty string failed");
    assert!(entries.is_empty());
}

// ---------------------------------------------------------------------------
// PortVlanResponse
// ---------------------------------------------------------------------------

fn port_vlan_json() -> String {
    let port = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":{id},"PVID":1,"Frame_Type":0}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| port(i)).collect();
    format!(r#"{{"PortNum":10,{}}}"#, ports.join(","))
}

#[test]
fn test_deserialize_port_vlan() {
    let json = port_vlan_json();
    let resp: PortVlanResponse =
        serde_json::from_str(&json).expect("PortVlanResponse deserialization failed");
    assert_eq!(resp.port_num, 10);
    assert_eq!(resp.port_1.port_id, 1);
    assert_eq!(resp.port_1.pvid, 1);
    assert_eq!(resp.port_1.frame_type, 0);
    assert_eq!(resp.port_10.port_id, 10);
}

#[test]
fn test_port_vlan_ports_returns_10() {
    let resp: PortVlanResponse = serde_json::from_str(&port_vlan_json()).unwrap();
    assert_eq!(resp.ports().len(), 10);
}

// ---------------------------------------------------------------------------
// IgmpConfig
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_igmp_config() {
    let json = r#"{"igmp":"on","fast_leave":"on","report_flood":"off"}"#;
    let cfg: IgmpConfig =
        serde_json::from_str(json).expect("IgmpConfig deserialization failed");
    assert_eq!(cfg.igmp, "on");
    assert_eq!(cfg.fast_leave, "on");
    assert_eq!(cfg.report_flood, "off");
}

// ---------------------------------------------------------------------------
// LoopStatusResponse
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_loop_status() {
    let json = r#"{
        "PortNum": "10",
        "Violdetd_1": "0", "Violdetd_2": "0", "Violdetd_3": "0",
        "Violdetd_4": "0", "Violdetd_5": "0", "Violdetd_6": "0",
        "Violdetd_7": "0", "Violdetd_8": "0", "Violdetd_9": "0",
        "Violdetd_10": "0"
    }"#;
    let ls: LoopStatusResponse =
        serde_json::from_str(json).expect("LoopStatusResponse deserialization failed");
    assert_eq!(ls.port_num, "10");
    assert_eq!(ls.viol_det_1, "0");
    assert_eq!(ls.viol_det_10, "0");
}

#[test]
fn test_loop_violations_all_zero_returns_empty() {
    let json = r#"{"PortNum":"10",
        "Violdetd_1":"0","Violdetd_2":"0","Violdetd_3":"0",
        "Violdetd_4":"0","Violdetd_5":"0","Violdetd_6":"0",
        "Violdetd_7":"0","Violdetd_8":"0","Violdetd_9":"0","Violdetd_10":"0"}"#;
    let ls: LoopStatusResponse = serde_json::from_str(json).unwrap();
    assert!(ls.violations().is_empty());
}

#[test]
fn test_loop_violations_non_zero() {
    let json = r#"{"PortNum":"10",
        "Violdetd_1":"0","Violdetd_2":"1","Violdetd_3":"0",
        "Violdetd_4":"0","Violdetd_5":"3","Violdetd_6":"0",
        "Violdetd_7":"0","Violdetd_8":"0","Violdetd_9":"0","Violdetd_10":"5"}"#;
    let ls: LoopStatusResponse = serde_json::from_str(json).unwrap();
    let violations = ls.violations();
    assert_eq!(violations.len(), 3);
    // Sorted by port number
    assert_eq!(violations[0], (2, &"1".to_string()));
    assert_eq!(violations[1], (5, &"3".to_string()));
    assert_eq!(violations[2], (10, &"5".to_string()));
}

// ---------------------------------------------------------------------------
// StpConfig
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_stp_config() {
    let json = r#"{
        "stp_enable": "0",
        "stp_rstp_mode": "RSTP",
        "num_ports": "10",
        "Port_1": {
            "Stp_Edge_1": "0",
            "Stp_Status_1": "Forward",
            "Hw_Port_Id_1": "Port 1"
        }
    }"#;
    let stp: StpConfig = serde_json::from_str(json).expect("StpConfig deserialization failed");
    assert_eq!(stp.stp_enable, "0");
    assert_eq!(stp.stp_rstp_mode, "RSTP");
    assert_eq!(stp.num_ports, "10");
    // The flattened Port_1 data ends up in `raw`
    assert!(stp.raw.get("Port_1").is_some());
}

// ---------------------------------------------------------------------------
// StormControlResponse
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_storm_control() {
    let json = r#"{
        "portnum": 10,
        "ports": [
            {"sctrl_bcast": 0, "sctrl_mcast": 0, "sctrl_unucast": 0, "sctrl_unmcast": 0, "port_id": 1},
            {"sctrl_bcast": 0, "sctrl_mcast": 0, "sctrl_unucast": 0, "sctrl_unmcast": 0, "port_id": 2}
        ]
    }"#;
    let sc: StormControlResponse =
        serde_json::from_str(json).expect("StormControlResponse deserialization failed");
    assert_eq!(sc.portnum, 10);
    assert_eq!(sc.ports.len(), 2);
    assert_eq!(sc.ports[0].port_id, 1);
    assert_eq!(sc.ports[0].sctrl_bcast, 0);
    assert_eq!(sc.ports[1].port_id, 2);
}

#[test]
fn test_deserialize_storm_control_all_ports() {
    // Realistic response with all 10 ports
    let ports_json: String = (1..=10)
        .map(|i| {
            format!(
                r#"{{"sctrl_bcast":0,"sctrl_mcast":0,"sctrl_unucast":0,"sctrl_unmcast":0,"port_id":{}}}"#,
                i
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let json = format!(r#"{{"portnum":10,"ports":[{}]}}"#, ports_json);
    let sc: StormControlResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(sc.ports.len(), 10);
    assert_eq!(sc.ports[9].port_id, 10);
}

// ---------------------------------------------------------------------------
// PortMirrorResponse
// ---------------------------------------------------------------------------

fn port_mirror_json() -> String {
    let port = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":"{id}","Ingress_Status":"Disabled","Egress_Status":"Disabled"}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| port(i)).collect();
    format!(
        r#"{{"PortNum":"10","MonitoringPortId":"0",{}}}"#,
        ports.join(",")
    )
}

#[test]
fn test_deserialize_port_mirror() {
    let json = port_mirror_json();
    let pm: PortMirrorResponse =
        serde_json::from_str(&json).expect("PortMirrorResponse deserialization failed");
    assert_eq!(pm.port_num, "10");
    assert_eq!(pm.monitoring_port_id, "0");
    assert_eq!(pm.port_1.port_id, "1");
    assert_eq!(pm.port_1.ingress_status, "Disabled");
    assert_eq!(pm.port_1.egress_status, "Disabled");
    assert_eq!(pm.port_10.port_id, "10");
}

// ---------------------------------------------------------------------------
// PortPvidsResponse
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_port_pvids() {
    let json = r#"{"port_pvids":[0,1,1,1,1,1,1,1,1,1,1]}"#;
    let pv: PortPvidsResponse =
        serde_json::from_str(json).expect("PortPvidsResponse deserialization failed");
    assert_eq!(pv.port_pvids.len(), 11);
    assert_eq!(pv.port_pvids[0], 0);
    assert_eq!(pv.port_pvids[1], 1);
    assert_eq!(pv.port_pvids[10], 1);
}

// ---------------------------------------------------------------------------
// TrunkConfigResponse
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_trunk_config() {
    let json = r#"{
        "PortNum": 10,
        "system_priority": 32768,
        "Port_1": {
            "portTypeId_1": 0,
            "portPriorityId_1": 128,
            "lacpTimeoutId_1": 0,
            "Port_1_grpInd": 0,
            "Port_1_state": 0
        },
        "Port_2": {
            "portTypeId_2": 0,
            "portPriorityId_2": 128,
            "lacpTimeoutId_2": 0,
            "Port_2_grpInd": 0,
            "Port_2_state": 1
        }
    }"#;
    let tc: TrunkConfigResponse =
        serde_json::from_str(json).expect("TrunkConfigResponse deserialization failed");
    assert_eq!(tc.port_num, 10);
    assert_eq!(tc.system_priority, 32768);
    // Flattened port configs end up in raw
    assert!(tc.raw.get("Port_1").is_some());
    assert!(tc.raw.get("Port_2").is_some());
}

// ---------------------------------------------------------------------------
// md5_hash
// ---------------------------------------------------------------------------

#[test]
fn test_md5_hash_admin() {
    assert_eq!(md5_hash("admin"), "21232f297a57a5a743894a0e4a801fc3");
}

#[test]
fn test_md5_hash_empty() {
    assert_eq!(
        md5_hash(""),
        "d41d8cd98f00b204e9800998ecf8427e"
    );
}

#[test]
fn test_md5_hash_password() {
    assert_eq!(
        md5_hash("password"),
        "5f4dcc3b5aa765d61d8327deb882cf99"
    );
}

#[test]
fn test_md5_hash_unicode() {
    // Ensure no panics on non-ASCII input
    let h = md5_hash("café");
    assert_eq!(h.len(), 32);
    assert!(h.chars().all(|c| c.is_ascii_hexdigit()));
}

// ---------------------------------------------------------------------------
// PortPvidsResponse edge cases
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_port_pvids_empty() {
    let json = r#"{"port_pvids":[]}"#;
    let pv: PortPvidsResponse = serde_json::from_str(json).unwrap();
    assert!(pv.port_pvids.is_empty());
}

#[test]
fn test_deserialize_port_pvids_single() {
    let json = r#"{"port_pvids":[42]}"#;
    let pv: PortPvidsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(pv.port_pvids.len(), 1);
    assert_eq!(pv.port_pvids[0], 42);
}

// ---------------------------------------------------------------------------
// serde_json::from_str::<T>() convenience — one call per model
// ---------------------------------------------------------------------------

#[test]
fn test_from_str_system_info() {
    let _: SystemInfo = serde_json::from_str(
        r#"{"temperature":"0","sys_ipv4":"0.0.0.0","sys_macaddr":"00:00:00:00:00:00","fw_ver":"1.0","hw_ver":"A0","des":"test"}"#,
    )
    .expect("SystemInfo from_str");
}

#[test]
fn test_from_str_network_settings() {
    let _: NetworkSettings = serde_json::from_str(
        r#"{"ipAddress":"0.0.0.0","netmask":"0.0.0.0","gateway":"0.0.0.0","dhcpEnabled":"0","dnsServer":"0.0.0.0","autoDnsEnabled":"0"}"#,
    )
    .expect("NetworkSettings from_str");
}

#[test]
fn test_from_str_port_settings() {
    let _: PortSettingsResponse =
        serde_json::from_str(&port_settings_json()).expect("PortSettingsResponse from_str");
}

#[test]
fn test_from_str_port_statistics() {
    let _: PortStatisticsResponse =
        serde_json::from_str(&port_stats_json()).expect("PortStatisticsResponse from_str");
}

#[test]
fn test_from_str_port_vlan() {
    let _: PortVlanResponse =
        serde_json::from_str(&port_vlan_json()).expect("PortVlanResponse from_str");
}

#[test]
fn test_from_str_igmp_config() {
    let _: IgmpConfig =
        serde_json::from_str(r#"{"igmp":"off","fast_leave":"off","report_flood":"on"}"#)
            .expect("IgmpConfig from_str");
}

#[test]
fn test_from_str_loop_status() {
    let _: LoopStatusResponse = serde_json::from_str(
        r#"{"PortNum":"10","Violdetd_1":"0","Violdetd_2":"0","Violdetd_3":"0","Violdetd_4":"0","Violdetd_5":"0","Violdetd_6":"0","Violdetd_7":"0","Violdetd_8":"0","Violdetd_9":"0","Violdetd_10":"0"}"#,
    )
    .expect("LoopStatusResponse from_str");
}

#[test]
fn test_from_str_stp_config() {
    let _: StpConfig = serde_json::from_str(
        r#"{"stp_enable":"1","stp_rstp_mode":"RSTP","num_ports":"10"}"#,
    )
    .expect("StpConfig from_str");
}

#[test]
fn test_from_str_storm_control() {
    let _: StormControlResponse =
        serde_json::from_str(r#"{"portnum":8,"ports":[]}"#)
            .expect("StormControlResponse from_str");
}

#[test]
fn test_from_str_port_mirror() {
    let _: PortMirrorResponse =
        serde_json::from_str(&port_mirror_json()).expect("PortMirrorResponse from_str");
}

#[test]
fn test_from_str_port_pvids() {
    let _: PortPvidsResponse =
        serde_json::from_str(r#"{"port_pvids":[1,1,1]}"#)
            .expect("PortPvidsResponse from_str");
}

#[test]
fn test_from_str_trunk_config() {
    let _: TrunkConfigResponse = serde_json::from_str(
        r#"{"PortNum":8,"system_priority":32768}"#,
    )
    .expect("TrunkConfigResponse from_str");
}

// ---------------------------------------------------------------------------
// MacEntry deserialization (standalone, not via data: prefix)
// ---------------------------------------------------------------------------

#[test]
fn test_deserialize_mac_entry_direct() {
    let json = r#"{
        "Dynamic_idx": 5,
        "Dynamic_mac_addr": "AA:BB:CC:DD:EE:FF",
        "Dynamic_vlan_id": 200,
        "Dynamic_fid": 1,
        "Dynamic_portid": 4,
        "Dynamic_age_timer": 123
    }"#;
    let entry: MacEntry = serde_json::from_str(json).expect("MacEntry deserialization failed");
    assert_eq!(entry.idx, 5);
    assert_eq!(entry.mac_addr, "AA:BB:CC:DD:EE:FF");
    assert_eq!(entry.vlan_id, 200);
    assert_eq!(entry.fid, 1);
    assert_eq!(entry.port_id, 4);
    assert_eq!(entry.age_timer, 123);
}

#[test]
fn test_deserialize_static_mac_entry_direct() {
    let json = r#"{
        "Static_idx": 3,
        "Static_mac_addr": "12:34:56:78:9A:BC",
        "Static_vlan_id": 50,
        "Static_portid": 7
    }"#;
    let entry: StaticMacEntry =
        serde_json::from_str(json).expect("StaticMacEntry deserialization failed");
    assert_eq!(entry.idx, 3);
    assert_eq!(entry.mac_addr, "12:34:56:78:9A:BC");
    assert_eq!(entry.vlan_id, 50);
    assert_eq!(entry.port_id, 7);
}

// ---------------------------------------------------------------------------
// Port statistics — zero / boundary values
// ---------------------------------------------------------------------------

#[test]
fn test_port_statistics_large_values() {
    let json = r#"{
        "PortNum": "10",
        "Port_1": {"Port_Id":"1","Port_Status":"Enabled","Link_Status":"10000MbpsFull","TxGoodPkt":"18446744073709551615","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_2": {"Port_Id":"2","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_3": {"Port_Id":"3","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_4": {"Port_Id":"4","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_5": {"Port_Id":"5","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_6": {"Port_Id":"6","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_7": {"Port_Id":"7","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_8": {"Port_Id":"8","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_9": {"Port_Id":"9","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"},
        "Port_10":{"Port_Id":"10","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"}
    }"#;
    let stats: PortStatisticsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(stats.port_1.link_status, "10000MbpsFull");
    // Large u64-as-string value
    assert_eq!(stats.port_1.tx_good_pkt, "18446744073709551615");
}
