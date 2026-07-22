//! Granular model unit tests for SKS3200 data types.
//!
//! Focuses on field-level assertions, edge cases, serialization roundtrips,
//! and default/zero-state behaviour for every model struct.

use sks3200::models::*;

// ---------------------------------------------------------------------------
// SystemInfo — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_system_info_all_fields() {
    let json = r#"{"temperature":"45","sys_ipv4":"10.0.0.1","sys_macaddr":"AA:BB:CC:DD:EE:FF","fw_ver":"1.2.3","hw_ver":"B1","des":"SKS3200-8E2X"}"#;
    let info: SystemInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.temperature, "45");
    assert_eq!(info.sys_ipv4, "10.0.0.1");
    assert_eq!(info.sys_macaddr, "AA:BB:CC:DD:EE:FF");
    assert_eq!(info.fw_ver, "1.2.3");
    assert_eq!(info.hw_ver, "B1");
    assert_eq!(info.des, "SKS3200-8E2X");
}

#[test]
fn test_system_info_roundtrip() {
    let info = SystemInfo {
        temperature: "42".to_string(),
        sys_ipv4: "192.168.1.1".to_string(),
        sys_macaddr: "00:11:22:33:44:55".to_string(),
        fw_ver: "2.0.0".to_string(),
        hw_ver: "A0".to_string(),
        des: "test".to_string(),
    };
    let json = serde_json::to_string(&info).unwrap();
    let deserialized: SystemInfo = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.temperature, info.temperature);
    assert_eq!(deserialized.sys_ipv4, info.sys_ipv4);
    assert_eq!(deserialized.sys_macaddr, info.sys_macaddr);
    assert_eq!(deserialized.fw_ver, info.fw_ver);
    assert_eq!(deserialized.hw_ver, info.hw_ver);
    assert_eq!(deserialized.des, info.des);
}

// ---------------------------------------------------------------------------
// NetworkSettings — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_network_settings_dhcp_enabled() {
    let json_on = r#"{"ipAddress":"10.0.0.1","netmask":"255.255.255.0","gateway":"10.0.0.254","dhcpEnabled":"1","dnsServer":"8.8.8.8","autoDnsEnabled":"0"}"#;
    let ns: NetworkSettings = serde_json::from_str(json_on).unwrap();
    assert_eq!(ns.dhcp_enabled, "1");
    assert_eq!(ns.auto_dns_enabled, "0");
}

#[test]
fn test_network_settings_empty_strings() {
    let json = r#"{"ipAddress":"","netmask":"","gateway":"","dhcpEnabled":"0","dnsServer":"","autoDnsEnabled":"0"}"#;
    let ns: NetworkSettings = serde_json::from_str(json).unwrap();
    assert_eq!(ns.ip_address, "");
    assert_eq!(ns.netmask, "");
    assert_eq!(ns.gateway, "");
}

#[test]
fn test_network_settings_roundtrip() {
    let orig = NetworkSettings {
        ip_address: "10.10.10.10".to_string(),
        netmask: "255.255.255.0".to_string(),
        gateway: "10.10.10.1".to_string(),
        dhcp_enabled: "0".to_string(),
        dns_server: "1.1.1.1".to_string(),
        auto_dns_enabled: "1".to_string(),
    };
    let json = serde_json::to_string(&orig).unwrap();
    let des: NetworkSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(des.ip_address, orig.ip_address);
    assert_eq!(des.netmask, orig.netmask);
    assert_eq!(des.gateway, orig.gateway);
    assert_eq!(des.dhcp_enabled, orig.dhcp_enabled);
    assert_eq!(des.dns_server, orig.dns_server);
    assert_eq!(des.auto_dns_enabled, orig.auto_dns_enabled);
}

// ---------------------------------------------------------------------------
// PortCfg / PortSettingsResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_port_cfg_eee_active() {
    let json = r#"{"EEE_Status":"eee_active","Port_Id":"1","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"1000MbpsFull","Flow_Ctrl_Cfg":"Off","Flow_Ctrl_Actual":"Off"}"#;
    let p: PortCfg = serde_json::from_str(json).unwrap();
    assert_eq!(p.eee_status, "eee_active");
    assert_eq!(p.spd_duplex_actual, "1000MbpsFull");
    assert_eq!(p.flow_ctrl_cfg, "Off");
}

#[test]
fn test_port_cfg_disabled_port() {
    let json = r#"{"EEE_Status":"eee_inactive","Port_Id":"1","Port_Status":"Disabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}"#;
    let p: PortCfg = serde_json::from_str(json).unwrap();
    assert_eq!(p.port_status, "Disabled");
}

#[test]
fn test_port_settings_ports_order() {
    // Verify that ports() returns items in the correct order (port_1 … port_10)
    let json = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"EEE_Status":"e","Port_Id":"{id}","Port_Status":"E","Spd_Duplex_Cfg":"A","Spd_Duplex_Actual":"LD","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| json(i)).collect();
    let full = format!(
        r#"{{"PortNum":"10","PortMode":"PORT_MODE_8_PLUS_2",{}}}"#,
        ports.join(",")
    );
    let resp: PortSettingsResponse = serde_json::from_str(&full).unwrap();
    let vec = resp.ports();
    assert_eq!(vec.len(), 10);
    for (i, port) in vec.iter().enumerate() {
        assert_eq!(
            port.port_id,
            (i + 1).to_string(),
            "port_id mismatch at index {}",
            i
        );
    }
}

#[test]
fn test_port_settings_roundtrip() {
    let json = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"EEE_Status":"e","Port_Id":"{id}","Port_Status":"E","Spd_Duplex_Cfg":"A","Spd_Duplex_Actual":"{spd}","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}}"#,
            id = id,
            spd = if id == 1 { "1000MbpsFull" } else { "Link Down" }
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| json(i)).collect();
    let full = format!(
        r#"{{"PortNum":"10","PortMode":"PORT_MODE_8_PLUS_2",{}}}"#,
        ports.join(",")
    );
    let orig: PortSettingsResponse = serde_json::from_str(&full).unwrap();
    let serialized = serde_json::to_string(&orig).unwrap();
    let des: PortSettingsResponse = serde_json::from_str(&serialized).unwrap();
    assert_eq!(des.port_num, orig.port_num);
    assert_eq!(des.port_mode, orig.port_mode);
    for (a, b) in orig.ports().iter().zip(des.ports().iter()) {
        assert_eq!(a.port_id, b.port_id);
        assert_eq!(a.spd_duplex_actual, b.spd_duplex_actual);
    }
}

#[test]
fn test_active_port_count_disabled_ports_not_counted() {
    // A port with Port_Status="Disabled" should NOT count even if speed is set
    let json = r#"{
        "PortNum":"10","PortMode":"PORT_MODE_8_PLUS_2",
        "Port_1":{"EEE_Status":"e","Port_Id":"1","Port_Status":"Disabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"1000MbpsFull","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_2":{"EEE_Status":"e","Port_Id":"2","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_3":{"EEE_Status":"e","Port_Id":"3","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_4":{"EEE_Status":"e","Port_Id":"4","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_5":{"EEE_Status":"e","Port_Id":"5","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_6":{"EEE_Status":"e","Port_Id":"6","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_7":{"EEE_Status":"e","Port_Id":"7","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_8":{"EEE_Status":"e","Port_Id":"8","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_9":{"EEE_Status":"e","Port_Id":"9","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"},
        "Port_10":{"EEE_Status":"e","Port_Id":"10","Port_Status":"Enabled","Spd_Duplex_Cfg":"Auto","Spd_Duplex_Actual":"Link Down","Flow_Ctrl_Cfg":"On","Flow_Ctrl_Actual":"On"}
    }"#;
    let resp: PortSettingsResponse = serde_json::from_str(json).unwrap();
    // Port_1 is Disabled (even though speed is set), others are Enabled but Link Down
    assert_eq!(resp.active_port_count(), 0);
}

// ---------------------------------------------------------------------------
// PortStats / PortStatisticsResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_port_stats_zero_packets() {
    let json = r#"{"Port_Id":"1","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"}"#;
    let ps: PortStats = serde_json::from_str(json).unwrap();
    assert_eq!(ps.tx_good_pkt, "0");
    assert_eq!(ps.tx_bad_pkt, "0");
    assert_eq!(ps.rx_good_pkt, "0");
    assert_eq!(ps.rx_bad_pkt, "0");
}

#[test]
fn test_port_stats_ports_order() {
    let json = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":"{id}","Port_Status":"Enabled","Link_Status":"LD","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| json(i)).collect();
    let full = format!(r#"{{"PortNum":"10",{}}}"#, ports.join(","));
    let resp: PortStatisticsResponse = serde_json::from_str(&full).unwrap();
    let vec = resp.ports();
    assert_eq!(vec.len(), 10);
    for (i, p) in vec.iter().enumerate() {
        assert_eq!(
            p.port_id,
            (i + 1).to_string(),
            "port_id mismatch at index {}",
            i
        );
    }
}

#[test]
fn test_port_statistics_roundtrip() {
    // Construct a known good instance, serialize, deserialize, compare
    let json = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":"{id}","Port_Status":"Enabled","Link_Status":"Link Down","TxGoodPkt":"0","TxBadPkt":"0","RxGoodPkt":"0","RxBadPkt":"0"}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| json(i)).collect();
    let full = format!(r#"{{"PortNum":"10",{}}}"#, ports.join(","));
    let orig: PortStatisticsResponse = serde_json::from_str(&full).unwrap();
    let serialized = serde_json::to_string(&orig).unwrap();
    let des: PortStatisticsResponse = serde_json::from_str(&serialized).unwrap();
    assert_eq!(des.port_num, orig.port_num);
    for (a, b) in orig.ports().iter().zip(des.ports().iter()) {
        assert_eq!(a.port_id, b.port_id);
        assert_eq!(a.link_status, b.link_status);
    }
}

// ---------------------------------------------------------------------------
// MacEntry — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_mac_entry_zero_values() {
    let json = r#"{"Dynamic_idx":0,"Dynamic_mac_addr":"00:00:00:00:00:00","Dynamic_vlan_id":0,"Dynamic_fid":0,"Dynamic_portid":0,"Dynamic_age_timer":0}"#;
    let entry: MacEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.idx, 0);
    assert_eq!(entry.mac_addr, "00:00:00:00:00:00");
    assert_eq!(entry.vlan_id, 0);
    assert_eq!(entry.fid, 0);
    assert_eq!(entry.port_id, 0);
    assert_eq!(entry.age_timer, 0);
}

#[test]
fn test_mac_entry_max_values() {
    let json = r#"{"Dynamic_idx":4294967295,"Dynamic_mac_addr":"FF:FF:FF:FF:FF:FF","Dynamic_vlan_id":4095,"Dynamic_fid":4294967295,"Dynamic_portid":4294967295,"Dynamic_age_timer":4294967295}"#;
    let entry: MacEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.idx, u32::MAX);
    assert_eq!(entry.vlan_id, 4095);
    assert_eq!(entry.fid, u32::MAX);
    assert_eq!(entry.port_id, u32::MAX);
    assert_eq!(entry.age_timer, u32::MAX);
}

#[test]
fn test_mac_entry_roundtrip() {
    let entry = MacEntry {
        idx: 42,
        mac_addr: "AA:BB:CC:DD:EE:FF".to_string(),
        vlan_id: 100,
        fid: 0,
        port_id: 8,
        age_timer: 300,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let des: MacEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(des.idx, entry.idx);
    assert_eq!(des.mac_addr, entry.mac_addr);
    assert_eq!(des.vlan_id, entry.vlan_id);
    assert_eq!(des.fid, entry.fid);
    assert_eq!(des.port_id, entry.port_id);
    assert_eq!(des.age_timer, entry.age_timer);
}

// ---------------------------------------------------------------------------
// StaticMacEntry — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_static_mac_entry_zero() {
    let json = r#"{"Static_idx":0,"Static_mac_addr":"00:00:00:00:00:00","Static_vlan_id":0,"Static_portid":0}"#;
    let entry: StaticMacEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.idx, 0);
    assert_eq!(entry.mac_addr, "00:00:00:00:00:00");
    assert_eq!(entry.vlan_id, 0);
    assert_eq!(entry.port_id, 0);
}

#[test]
fn test_static_mac_entry_roundtrip() {
    let entry = StaticMacEntry {
        idx: 7,
        mac_addr: "11:22:33:44:55:66".to_string(),
        vlan_id: 999,
        port_id: 5,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let des: StaticMacEntry = serde_json::from_str(&json).unwrap();
    assert_eq!(des.idx, entry.idx);
    assert_eq!(des.mac_addr, entry.mac_addr);
    assert_eq!(des.vlan_id, entry.vlan_id);
    assert_eq!(des.port_id, entry.port_id);
}

// ---------------------------------------------------------------------------
// PortVlanEntry / PortVlanResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_port_vlan_entry_zero() {
    let json = r#"{"Port_Id":0,"PVID":0,"Frame_Type":0}"#;
    let entry: PortVlanEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.port_id, 0);
    assert_eq!(entry.pvid, 0);
    assert_eq!(entry.frame_type, 0);
}

#[test]
fn test_port_vlan_entry_different_pvid() {
    let json = r#"{"Port_Id":5,"PVID":100,"Frame_Type":1}"#;
    let entry: PortVlanEntry = serde_json::from_str(json).unwrap();
    assert_eq!(entry.port_id, 5);
    assert_eq!(entry.pvid, 100);
    assert_eq!(entry.frame_type, 1);
}

#[test]
fn test_port_vlan_ports_order() {
    let json = |id: u32| -> String {
        format!(
            r#""Port_{id}":{{"Port_Id":{id},"PVID":1,"Frame_Type":0}}"#,
            id = id
        )
    };
    let ports: Vec<String> = (1..=10).map(|i| json(i)).collect();
    let full = format!(r#"{{"PortNum":10,{}}}"#, ports.join(","));
    let resp: PortVlanResponse = serde_json::from_str(&full).unwrap();
    for (i, port) in resp.ports().iter().enumerate() {
        assert_eq!(port.port_id, (i + 1) as u32);
    }
}

// ---------------------------------------------------------------------------
// IgmpConfig — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_igmp_config_all_off() {
    let json = r#"{"igmp":"off","fast_leave":"off","report_flood":"off"}"#;
    let cfg: IgmpConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.igmp, "off");
    assert_eq!(cfg.fast_leave, "off");
    assert_eq!(cfg.report_flood, "off");
}

#[test]
fn test_igmp_config_all_on() {
    let json = r#"{"igmp":"on","fast_leave":"on","report_flood":"on"}"#;
    let cfg: IgmpConfig = serde_json::from_str(json).unwrap();
    assert_eq!(cfg.igmp, "on");
    assert_eq!(cfg.fast_leave, "on");
    assert_eq!(cfg.report_flood, "on");
}

#[test]
fn test_igmp_config_roundtrip() {
    let cfg = IgmpConfig {
        igmp: "on".to_string(),
        fast_leave: "off".to_string(),
        report_flood: "on".to_string(),
    };
    let json = serde_json::to_string(&cfg).unwrap();
    let des: IgmpConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(des.igmp, cfg.igmp);
    assert_eq!(des.fast_leave, cfg.fast_leave);
    assert_eq!(des.report_flood, cfg.report_flood);
}

// ---------------------------------------------------------------------------
// LoopStatusResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_loop_status_mixed_violations() {
    let json = r#"{"PortNum":"10",
        "Violdetd_1":"0","Violdetd_2":"0","Violdetd_3":"1",
        "Violdetd_4":"0","Violdetd_5":"2","Violdetd_6":"0",
        "Violdetd_7":"0","Violdetd_8":"0","Violdetd_9":"9","Violdetd_10":"0"}"#;
    let ls: LoopStatusResponse = serde_json::from_str(json).unwrap();
    let v = ls.violations();
    assert_eq!(v.len(), 3);
    assert_eq!(v[0], (3, &"1".to_string()));
    assert_eq!(v[1], (5, &"2".to_string()));
    assert_eq!(v[2], (9, &"9".to_string()));
}

#[test]
fn test_loop_status_all_violations() {
    // Every port has a violation
    let viols: Vec<String> = (1..=10)
        .map(|i| format!(r#""Violdetd_{i}":"{i}""#, i = i))
        .collect();
    let json = format!(r#"{{"PortNum":"10",{}}}"#, viols.join(","));
    let ls: LoopStatusResponse = serde_json::from_str(&json).unwrap();
    let v = ls.violations();
    assert_eq!(v.len(), 10);
    for i in 0..10 {
        assert_eq!(v[i].0, (i + 1) as u32);
        assert_eq!(*v[i].1, (i + 1).to_string());
    }
}

#[test]
fn test_loop_status_roundtrip() {
    let ls = LoopStatusResponse {
        port_num: "10".to_string(),
        viol_det_1: "0".to_string(),
        viol_det_2: "1".to_string(),
        viol_det_3: "0".to_string(),
        viol_det_4: "0".to_string(),
        viol_det_5: "0".to_string(),
        viol_det_6: "0".to_string(),
        viol_det_7: "0".to_string(),
        viol_det_8: "0".to_string(),
        viol_det_9: "0".to_string(),
        viol_det_10: "0".to_string(),
    };
    let json = serde_json::to_string(&ls).unwrap();
    let des: LoopStatusResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(des.port_num, ls.port_num);
    assert_eq!(des.viol_det_1, ls.viol_det_1);
    assert_eq!(des.viol_det_2, ls.viol_det_2);
    assert_eq!(des.violations().len(), 1);
    assert_eq!(des.violations()[0].0, 2);
}

// ---------------------------------------------------------------------------
// StpConfig — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_stp_config_stp_enabled() {
    let json = r#"{"stp_enable":"1","stp_rstp_mode":"RSTP","num_ports":"10"}"#;
    let stp: StpConfig = serde_json::from_str(json).unwrap();
    assert_eq!(stp.stp_enable, "1");
    assert_eq!(stp.stp_rstp_mode, "RSTP");
    assert_eq!(stp.num_ports, "10");
}

#[test]
fn test_stp_config_stp_disabled() {
    let json = r#"{"stp_enable":"0","stp_rstp_mode":"RSTP","num_ports":"10"}"#;
    let stp: StpConfig = serde_json::from_str(json).unwrap();
    assert_eq!(stp.stp_enable, "0");
}

#[test]
fn test_stp_config_roundtrip() {
    let stp = StpConfig {
        stp_enable: "1".to_string(),
        stp_rstp_mode: "MSTP".to_string(),
        num_ports: "8".to_string(),
        raw: serde_json::json!({
            "Port_1": {"Stp_Edge_1": "0", "Stp_Status_1": "Forward", "Hw_Port_Id_1": "Port 1"}
        }),
    };
    let json = serde_json::to_string(&stp).unwrap();
    let des: StpConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(des.stp_enable, stp.stp_enable);
    assert_eq!(des.stp_rstp_mode, stp.stp_rstp_mode);
    assert_eq!(des.num_ports, stp.num_ports);
    // The raw Port_1 data should survive roundtrip via flatten
    assert_eq!(des.raw["Port_1"]["Stp_Status_1"], "Forward");
}

// ---------------------------------------------------------------------------
// StormControlResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_storm_control_empty_ports() {
    let json = r#"{"portnum":0,"ports":[]}"#;
    let sc: StormControlResponse = serde_json::from_str(json).unwrap();
    assert_eq!(sc.portnum, 0);
    assert!(sc.ports.is_empty());
}

#[test]
fn test_storm_control_non_zero_values() {
    let json = r#"{
        "portnum": 10,
        "ports": [
            {"sctrl_bcast": 100, "sctrl_mcast": 50, "sctrl_unucast": 25, "sctrl_unmcast": 10, "port_id": 1},
            {"sctrl_bcast": 200, "sctrl_mcast": 150, "sctrl_unucast": 75, "sctrl_unmcast": 30, "port_id": 2}
        ]
    }"#;
    let sc: StormControlResponse = serde_json::from_str(json).unwrap();
    assert_eq!(sc.portnum, 10);
    assert_eq!(sc.ports[0].sctrl_bcast, 100);
    assert_eq!(sc.ports[0].sctrl_mcast, 50);
    assert_eq!(sc.ports[0].sctrl_unucast, 25);
    assert_eq!(sc.ports[0].sctrl_unmcast, 10);
    assert_eq!(sc.ports[1].sctrl_bcast, 200);
}

#[test]
fn test_storm_control_roundtrip() {
    let sc = StormControlResponse {
        portnum: 8,
        ports: vec![
            StormControlPort {
                port_id: 1,
                sctrl_bcast: 0,
                sctrl_mcast: 0,
                sctrl_unucast: 0,
                sctrl_unmcast: 0,
            },
            StormControlPort {
                port_id: 2,
                sctrl_bcast: 50,
                sctrl_mcast: 25,
                sctrl_unucast: 10,
                sctrl_unmcast: 5,
            },
        ],
    };
    let json = serde_json::to_string(&sc).unwrap();
    let des: StormControlResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(des.portnum, sc.portnum);
    assert_eq!(des.ports.len(), sc.ports.len());
    for (a, b) in sc.ports.iter().zip(des.ports.iter()) {
        assert_eq!(a.port_id, b.port_id);
        assert_eq!(a.sctrl_bcast, b.sctrl_bcast);
        assert_eq!(a.sctrl_mcast, b.sctrl_mcast);
        assert_eq!(a.sctrl_unucast, b.sctrl_unucast);
        assert_eq!(a.sctrl_unmcast, b.sctrl_unmcast);
    }
}

// ---------------------------------------------------------------------------
// PortMirrorEntry / PortMirrorResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_port_mirror_entry_ingress_egress_mixed() {
    let json = r#"{"Port_Id":"1","Ingress_Status":"Enabled","Egress_Status":"Disabled"}"#;
    let pme: PortMirrorEntry = serde_json::from_str(json).unwrap();
    assert_eq!(pme.port_id, "1");
    assert_eq!(pme.ingress_status, "Enabled");
    assert_eq!(pme.egress_status, "Disabled");
}

#[test]
fn test_port_mirror_all_enabled() {
    let json = r#"{"Port_Id":"2","Ingress_Status":"Enabled","Egress_Status":"Enabled"}"#;
    let pme: PortMirrorEntry = serde_json::from_str(json).unwrap();
    assert_eq!(pme.port_id, "2");
    assert_eq!(pme.ingress_status, "Enabled");
    assert_eq!(pme.egress_status, "Enabled");
}

#[test]
fn test_port_mirror_with_monitoring_port() {
    let ports: Vec<String> = (1..=10)
        .map(|i| {
            format!(
                r#""Port_{i}":{{"Port_Id":"{i}","Ingress_Status":"Disabled","Egress_Status":"Disabled"}}"#,
                i = i
            )
        })
        .collect();
    let json = format!(
        r#"{{"PortNum":"10","MonitoringPortId":"5",{}}}"#,
        ports.join(",")
    );
    let pm: PortMirrorResponse = serde_json::from_str(&json).unwrap();
    assert_eq!(pm.monitoring_port_id, "5");
}

// ---------------------------------------------------------------------------
// PortPvidsResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_port_pvids_all_zeros() {
    let json = r#"{"port_pvids":[0,0,0,0,0,0,0,0,0,0,0]}"#;
    let pv: PortPvidsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(pv.port_pvids.len(), 11);
    assert!(pv.port_pvids.iter().all(|&v| v == 0));
}

#[test]
fn test_port_pvids_mixed_values() {
    let json = r#"{"port_pvids":[0,1,2,3,4,5,6,7,8,9,10]}"#;
    let pv: PortPvidsResponse = serde_json::from_str(json).unwrap();
    assert_eq!(pv.port_pvids[0], 0);
    assert_eq!(pv.port_pvids[5], 5);
    assert_eq!(pv.port_pvids[10], 10);
}

// ---------------------------------------------------------------------------
// TrunkConfigResponse — field-level
// ---------------------------------------------------------------------------

#[test]
fn test_trunk_config_different_priorities() {
    let json = r#"{
        "PortNum": 10,
        "system_priority": 4096,
        "Port_1": {"portTypeId_1":1,"portPriorityId_1":64,"lacpTimeoutId_1":1,"Port_1_grpInd":0,"Port_1_state":0},
        "Port_2": {"portTypeId_2":0,"portPriorityId_2":255,"lacpTimeoutId_2":0,"Port_2_grpInd":1,"Port_2_state":1}
    }"#;
    let tc: TrunkConfigResponse = serde_json::from_str(json).unwrap();
    assert_eq!(tc.port_num, 10);
    assert_eq!(tc.system_priority, 4096);
    let port1 = &tc.raw["Port_1"];
    assert_eq!(port1["portPriorityId_1"], 64);
    assert_eq!(port1["lacpTimeoutId_1"], 1);
    let port2 = &tc.raw["Port_2"];
    assert_eq!(port2["portPriorityId_2"], 255);
    assert_eq!(port2["Port_2_grpInd"], 1);
}

// ---------------------------------------------------------------------------
// Edge cases common to all models
// ---------------------------------------------------------------------------

#[test]
fn test_unknown_fields_ignored() {
    // Models should ignore extra unknown fields (serde default behaviour)
    let json = r#"{"temperature":"25","sys_ipv4":"1.2.3.4","sys_macaddr":"00:00:00:00:00:00","fw_ver":"1.0","hw_ver":"A0","des":"test","extra_field":"ignored"}"#;
    let info: SystemInfo = serde_json::from_str(json).unwrap();
    assert_eq!(info.temperature, "25");
}

#[test]
fn test_missing_required_field_fails() {
    // Temperature is required — absence should fail
    let json = r#"{"sys_ipv4":"1.2.3.4","sys_macaddr":"00:00:00:00:00:00","fw_ver":"1.0","hw_ver":"A0","des":"test"}"#;
    let result: Result<SystemInfo, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Expected deserialization to fail when field is missing"
    );
}

#[test]
fn test_null_fields_fail() {
    // serde expects the field types (String etc.), not null
    let json = r#"{"temperature":null,"sys_ipv4":"1.2.3.4","sys_macaddr":"00:00:00:00:00:00","fw_ver":"1.0","hw_ver":"A0","des":"test"}"#;
    let result: Result<SystemInfo, _> = serde_json::from_str(json);
    assert!(
        result.is_err(),
        "Expected deserialization to fail on null field"
    );
}
