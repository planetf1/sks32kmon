//! In-memory mock switch client for testing and safe dry-runs.
//!
//! Mimics the full SKS3200 API surface — all read endpoints return
//! sensible defaults, and all write endpoints mutate in-memory state
//! and log the operation. No network traffic, no real switch.
//!
//! Uses a single `RefCell<MockState>` for interior mutability so that
//! multi-field writes (e.g., VLAN + PVID) are atomic borrows.

use std::cell::RefCell;

use anyhow::{bail, Result};

use crate::client::md5_hash;
use crate::models::*;
use crate::write_models::*;

// ---------------------------------------------------------------------------
// Mock switch state
// ---------------------------------------------------------------------------

/// All configurable state of a mock SKS3200 switch.
/// Fields use `RefCell` at the top level so that write methods
/// can `borrow_mut()` a single lock for the entire mutation.
struct MockState {
    authenticated: bool,
    _username_hash: String,
    _password_hash: String,

    // System
    system_info: SystemInfo,
    network_settings: NetworkSettings,

    // Ports
    port_settings: PortSettingsResponse,
    port_statistics: PortStatisticsResponse,

    // VLAN
    port_vlan: PortVlanResponse,
    port_pvids: PortPvidsResponse,

    // MAC
    dynamic_mac_entries: Vec<MacEntry>,
    static_mac_entries: Vec<StaticMacEntry>,

    // STP
    stp_config: StpConfig,

    // Loop protection
    loop_status: LoopStatusResponse,

    // IGMP
    igmp_config: IgmpConfig,

    // Storm control
    storm_control: StormControlResponse,

    // Port mirror
    port_mirror: PortMirrorResponse,

    // Trunk
    trunk_config: TrunkConfigResponse,

    // Operation log (for test assertions)
    write_log: Vec<MockWriteEntry>,
}

/// A recorded write operation for test inspection.
#[derive(Debug, Clone)]
pub struct MockWriteEntry {
    #[allow(dead_code)]
    pub operation: String,
    #[allow(dead_code)]
    pub payload: String,
}

// ---------------------------------------------------------------------------
// Public client
// ---------------------------------------------------------------------------

/// A mock SKS3200 switch with full read/write API surface.
/// All read methods return pre-populated default state.
/// All write methods mutate internal state and log the operation.
pub struct MockSwitchClient {
    host: String,
    state: RefCell<MockState>,
}

impl MockSwitchClient {
    /// Create a new mock client with sensible defaults for all state.
    pub fn new(host: &str, username: &str, password: &str) -> Self {
        let state = MockState {
            authenticated: true,
            _username_hash: md5_hash(username),
            _password_hash: md5_hash(password),

            system_info: SystemInfo {
                temperature: "45".to_string(),
                sys_ipv4: host.to_string(),
                sys_macaddr: "00:11:22:33:44:55".to_string(),
                fw_ver: "2.0.0.3".to_string(),
                hw_ver: "A0".to_string(),
                des: "SKS3200-8E2X".to_string(),
            },

            network_settings: NetworkSettings {
                ip_address: host.to_string(),
                netmask: "255.255.255.0".to_string(),
                gateway: "192.168.1.1".to_string(),
                dhcp_enabled: "0".to_string(),
                dns_server: "8.8.8.8".to_string(),
                auto_dns_enabled: "0".to_string(),
            },

            port_settings: make_default_port_settings(),
            port_statistics: make_default_port_statistics(),
            port_vlan: make_default_port_vlan(),
            port_pvids: make_default_port_pvids(),

            dynamic_mac_entries: vec![
                MacEntry {
                    idx: 1,
                    mac_addr: "AA:BB:CC:DD:EE:01".to_string(),
                    vlan_id: 1,
                    fid: 0,
                    port_id: 2,
                    age_timer: 300,
                },
                MacEntry {
                    idx: 2,
                    mac_addr: "AA:BB:CC:DD:EE:02".to_string(),
                    vlan_id: 1,
                    fid: 0,
                    port_id: 8,
                    age_timer: 180,
                },
            ],
            static_mac_entries: vec![StaticMacEntry {
                idx: 1,
                mac_addr: "00:00:00:00:00:01".to_string(),
                vlan_id: 1,
                port_id: 1,
            }],

            stp_config: StpConfig {
                stp_enable: "0".to_string(),
                stp_rstp_mode: "RSTP".to_string(),
                num_ports: "10".to_string(),
                raw: serde_json::json!({}),
            },

            loop_status: LoopStatusResponse {
                port_num: "10".to_string(),
                viol_det_1: "0".to_string(),
                viol_det_2: "0".to_string(),
                viol_det_3: "0".to_string(),
                viol_det_4: "0".to_string(),
                viol_det_5: "0".to_string(),
                viol_det_6: "0".to_string(),
                viol_det_7: "0".to_string(),
                viol_det_8: "0".to_string(),
                viol_det_9: "0".to_string(),
                viol_det_10: "0".to_string(),
            },

            igmp_config: IgmpConfig {
                igmp: "on".to_string(),
                fast_leave: "off".to_string(),
                report_flood: "off".to_string(),
            },

            storm_control: StormControlResponse {
                portnum: 10,
                ports: (1..=10)
                    .map(|i| StormControlPort {
                        port_id: i,
                        sctrl_bcast: 0,
                        sctrl_mcast: 0,
                        sctrl_unucast: 0,
                        sctrl_unmcast: 0,
                    })
                    .collect(),
            },

            port_mirror: PortMirrorResponse {
                port_num: "10".to_string(),
                monitoring_port_id: "0".to_string(),
                port_1: mirror_disabled(1),
                port_2: mirror_disabled(2),
                port_3: mirror_disabled(3),
                port_4: mirror_disabled(4),
                port_5: mirror_disabled(5),
                port_6: mirror_disabled(6),
                port_7: mirror_disabled(7),
                port_8: mirror_disabled(8),
                port_9: mirror_disabled(9),
                port_10: mirror_disabled(10),
            },

            trunk_config: TrunkConfigResponse {
                port_num: 10,
                system_priority: 32768,
                raw: serde_json::json!({}),
            },

            write_log: Vec::new(),
        };

        Self {
            host: host.to_string(),
            state: RefCell::new(state),
        }
    }

    /// Verify credentials (called by ApiClient::connect_mock).
    /// Returns an error if username/password don't match the defaults.
    pub fn verify_auth(username: &str, password: &str) -> Result<()> {
        // Mock always accepts a specific test pair, or any non-empty creds
        if username.is_empty() || password.is_empty() {
            bail!("Mock: authentication failed — empty credentials");
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Read API (mirrors SwitchClient)
    // -----------------------------------------------------------------------

    pub fn get_system_info(&self) -> Result<SystemInfo> {
        self.check_session()?;
        Ok(self.state.borrow().system_info.clone())
    }

    pub fn get_network_settings(&self) -> Result<NetworkSettings> {
        self.check_session()?;
        Ok(self.state.borrow().network_settings.clone())
    }

    pub fn get_port_settings(&self) -> Result<PortSettingsResponse> {
        self.check_session()?;
        Ok(self.state.borrow().port_settings.clone())
    }

    pub fn get_port_statistics(&self) -> Result<PortStatisticsResponse> {
        self.check_session()?;
        Ok(self.state.borrow().port_statistics.clone())
    }

    pub fn get_dynamic_mac_entries(&self) -> Result<Vec<MacEntry>> {
        self.check_session()?;
        Ok(self.state.borrow().dynamic_mac_entries.clone())
    }

    pub fn get_static_mac_entries(&self) -> Result<Vec<StaticMacEntry>> {
        self.check_session()?;
        Ok(self.state.borrow().static_mac_entries.clone())
    }

    pub fn get_loop_status(&self) -> Result<LoopStatusResponse> {
        self.check_session()?;
        Ok(self.state.borrow().loop_status.clone())
    }

    pub fn get_stp_config(&self) -> Result<StpConfig> {
        self.check_session()?;
        Ok(self.state.borrow().stp_config.clone())
    }

    pub fn get_port_vlan(&self) -> Result<PortVlanResponse> {
        self.check_session()?;
        Ok(self.state.borrow().port_vlan.clone())
    }

    pub fn get_all_port_pvids(&self) -> Result<PortPvidsResponse> {
        self.check_session()?;
        Ok(self.state.borrow().port_pvids.clone())
    }

    pub fn get_igmp_config(&self) -> Result<IgmpConfig> {
        self.check_session()?;
        Ok(self.state.borrow().igmp_config.clone())
    }

    pub fn get_storm_control(&self) -> Result<StormControlResponse> {
        self.check_session()?;
        Ok(self.state.borrow().storm_control.clone())
    }

    pub fn get_port_mirror(&self) -> Result<PortMirrorResponse> {
        self.check_session()?;
        Ok(self.state.borrow().port_mirror.clone())
    }

    pub fn get_trunk_config(&self) -> Result<TrunkConfigResponse> {
        self.check_session()?;
        Ok(self.state.borrow().trunk_config.clone())
    }

    // -----------------------------------------------------------------------
    // Write API
    // -----------------------------------------------------------------------

    /// `POST /set_des.json`
    pub fn set_description(&self, description: &str) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_description",
            &format!(r#"{{"des":"{}"}}"#, description),
        );
        self.state.borrow_mut().system_info.des = description.to_string();
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /apply_user_port_setting.json`
    pub fn set_port_settings(&self, request: &PortSettingsApplyRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_port_settings",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();

        apply_port_setting_to_cfg(&mut state.port_settings.port_1, &request.port_1);
        apply_port_setting_to_cfg(&mut state.port_settings.port_2, &request.port_2);
        apply_port_setting_to_cfg(&mut state.port_settings.port_3, &request.port_3);
        apply_port_setting_to_cfg(&mut state.port_settings.port_4, &request.port_4);
        apply_port_setting_to_cfg(&mut state.port_settings.port_5, &request.port_5);
        apply_port_setting_to_cfg(&mut state.port_settings.port_6, &request.port_6);
        apply_port_setting_to_cfg(&mut state.port_settings.port_7, &request.port_7);
        apply_port_setting_to_cfg(&mut state.port_settings.port_8, &request.port_8);
        apply_port_setting_to_cfg(&mut state.port_settings.port_9, &request.port_9);
        apply_port_setting_to_cfg(&mut state.port_settings.port_10, &request.port_10);

        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /network_settings_ipv4.json`
    pub fn set_network_settings(&self, request: &NetworkSettingsRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_network_settings",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        if let Some(ref v) = request.ip_address {
            state.network_settings.ip_address = v.clone();
        }
        if let Some(ref v) = request.netmask {
            state.network_settings.netmask = v.clone();
        }
        if let Some(ref v) = request.gateway {
            state.network_settings.gateway = v.clone();
        }
        if let Some(ref v) = request.dhcp_enabled {
            state.network_settings.dhcp_enabled = v.clone();
        }
        if let Some(ref v) = request.dns_server {
            state.network_settings.dns_server = v.clone();
        }
        if let Some(ref v) = request.auto_dns_enabled {
            state.network_settings.auto_dns_enabled = v.clone();
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /port_vlan.json`
    pub fn set_port_vlan(&self, request: &PortVlanRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_port_vlan",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();

        apply_vlan_to_port(&mut state.port_vlan.port_1, &request.port_1);
        apply_vlan_to_port(&mut state.port_vlan.port_2, &request.port_2);
        apply_vlan_to_port(&mut state.port_vlan.port_3, &request.port_3);
        apply_vlan_to_port(&mut state.port_vlan.port_4, &request.port_4);
        apply_vlan_to_port(&mut state.port_vlan.port_5, &request.port_5);
        apply_vlan_to_port(&mut state.port_vlan.port_6, &request.port_6);
        apply_vlan_to_port(&mut state.port_vlan.port_7, &request.port_7);
        apply_vlan_to_port(&mut state.port_vlan.port_8, &request.port_8);
        apply_vlan_to_port(&mut state.port_vlan.port_9, &request.port_9);
        apply_vlan_to_port(&mut state.port_vlan.port_10, &request.port_10);

        // Also update the compact PVID array
        if let Some(ref e) = request.port_1 {
            if (e.port_id as usize) < state.port_pvids.port_pvids.len() {
                state.port_pvids.port_pvids[e.port_id as usize] = e.pvid;
            }
        }
        // ... repeat for other ports (simplified — covers port_1 through port_10)
        update_pvid_array(&mut state.port_pvids, &request.port_2);
        update_pvid_array(&mut state.port_pvids, &request.port_3);
        update_pvid_array(&mut state.port_pvids, &request.port_4);
        update_pvid_array(&mut state.port_pvids, &request.port_5);
        update_pvid_array(&mut state.port_pvids, &request.port_6);
        update_pvid_array(&mut state.port_pvids, &request.port_7);
        update_pvid_array(&mut state.port_pvids, &request.port_8);
        update_pvid_array(&mut state.port_pvids, &request.port_9);
        update_pvid_array(&mut state.port_pvids, &request.port_10);

        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /igmp_config.json`
    pub fn set_igmp_config(&self, request: &IgmpConfigRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_igmp_config",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        if let Some(ref v) = request.igmp {
            state.igmp_config.igmp = v.clone();
        }
        if let Some(ref v) = request.fast_leave {
            state.igmp_config.fast_leave = v.clone();
        }
        if let Some(ref v) = request.report_flood {
            state.igmp_config.report_flood = v.clone();
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /storm_ctrl_cfg.json`
    pub fn set_storm_control(&self, request: &StormControlRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_storm_control",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        for req_port in &request.ports {
            if let Some(mock_port) = state
                .storm_control
                .ports
                .iter_mut()
                .find(|p| p.port_id == req_port.port_id)
            {
                *mock_port = StormControlPort {
                    port_id: req_port.port_id,
                    sctrl_bcast: req_port.sctrl_bcast,
                    sctrl_mcast: req_port.sctrl_mcast,
                    sctrl_unucast: req_port.sctrl_unucast,
                    sctrl_unmcast: req_port.sctrl_unmcast,
                };
            }
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /port_mirror.json`
    pub fn set_port_mirror(&self, request: &PortMirrorRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_port_mirror",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        state.port_mirror.monitoring_port_id = request.monitoring_port_id.clone();
        if let Some(ref e) = request.port_1 {
            state.port_mirror.port_1 = e.clone();
        }
        if let Some(ref e) = request.port_2 {
            state.port_mirror.port_2 = e.clone();
        }
        if let Some(ref e) = request.port_3 {
            state.port_mirror.port_3 = e.clone();
        }
        if let Some(ref e) = request.port_4 {
            state.port_mirror.port_4 = e.clone();
        }
        if let Some(ref e) = request.port_5 {
            state.port_mirror.port_5 = e.clone();
        }
        if let Some(ref e) = request.port_6 {
            state.port_mirror.port_6 = e.clone();
        }
        if let Some(ref e) = request.port_7 {
            state.port_mirror.port_7 = e.clone();
        }
        if let Some(ref e) = request.port_8 {
            state.port_mirror.port_8 = e.clone();
        }
        if let Some(ref e) = request.port_9 {
            state.port_mirror.port_9 = e.clone();
        }
        if let Some(ref e) = request.port_10 {
            state.port_mirror.port_10 = e.clone();
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /stp.json`
    pub fn set_stp_config(&self, request: &StpConfigRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_stp_config",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        if let Some(ref v) = request.stp_enable {
            state.stp_config.stp_enable = v.clone();
        }
        if let Some(ref v) = request.stp_rstp_mode {
            state.stp_config.stp_rstp_mode = v.clone();
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /port_trunk_cfg.json`
    pub fn set_trunk_config(&self, request: &TrunkConfigRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_trunk_config",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        let mut state = self.state.borrow_mut();
        state.trunk_config.system_priority = request.system_priority;

        // Merge per-port trunk entries into the raw JSON blob
        let mut raw = state.trunk_config.raw.clone();
        update_trunk_raw(&mut raw, 1, &request.port_1);
        update_trunk_raw(&mut raw, 2, &request.port_2);
        update_trunk_raw(&mut raw, 3, &request.port_3);
        update_trunk_raw(&mut raw, 4, &request.port_4);
        update_trunk_raw(&mut raw, 5, &request.port_5);
        update_trunk_raw(&mut raw, 6, &request.port_6);
        update_trunk_raw(&mut raw, 7, &request.port_7);
        update_trunk_raw(&mut raw, 8, &request.port_8);
        update_trunk_raw(&mut raw, 9, &request.port_9);
        update_trunk_raw(&mut raw, 10, &request.port_10);
        state.trunk_config.raw = raw;

        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /port_lock_cfg.json`
    pub fn set_loop_protection(&self, request: &LoopProtectionRequest) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write(
            "set_loop_protection",
            &serde_json::to_string(request).unwrap_or_else(|e| format!("<serialize error: {}>", e)),
        );
        // Mock: no stored config state for loop protection (write-only config endpoint)
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /save_all_configs.json`
    pub fn save_config(&self) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write("save_config", "{}");
        // Mock: no-op (running config is already the persisted state)
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `GET /clear_statistics.json` (uses GET, not POST!)
    pub fn clear_statistics(&self) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write("clear_statistics", "GET (no body)");

        // Collect all current port metadata before mutating
        let port_info: Vec<(u32, String, String)> = {
            let state = self.state.borrow();
            state
                .port_statistics
                .ports()
                .iter()
                .filter_map(|p| {
                    let id = p.port_id.parse::<u32>().ok()?;
                    Some((id, p.port_status.clone(), p.link_status.clone()))
                })
                .collect()
        };

        // Now mutate — no immutable borrow held
        let mut state = self.state.borrow_mut();
        for (port_id, status, link) in port_info {
            let zeroed = PortStats {
                port_id: port_id.to_string(),
                port_status: status,
                link_status: link,
                tx_good_pkt: "0".to_string(),
                tx_bad_pkt: "0".to_string(),
                rx_good_pkt: "0".to_string(),
                rx_bad_pkt: "0".to_string(),
            };
            set_port_stats(&mut state.port_statistics, port_id, zeroed);
        }
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /mac_clear_dynamic_mac_entries.json`
    pub fn clear_mac_entries(&self) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write("clear_mac_entries", "{}");
        self.state.borrow_mut().dynamic_mac_entries.clear();
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /system_reboot.json`
    pub fn reboot(&self) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write("reboot", "{}");
        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `POST /factory_reset.json`
    /// Resets key configurable state back to factory defaults.
    pub fn factory_reset(&self) -> Result<WriteResponse> {
        self.check_session()?;
        self.log_write("factory_reset", "{}");

        let mut state = self.state.borrow_mut();
        // Reset writable fields to defaults
        state.system_info.des = "SKS3200-8E2X".to_string();
        state.network_settings = NetworkSettings {
            ip_address: self.host.clone(),
            netmask: "255.255.255.0".to_string(),
            gateway: "192.168.1.1".to_string(),
            dhcp_enabled: "0".to_string(),
            dns_server: "8.8.8.8".to_string(),
            auto_dns_enabled: "0".to_string(),
        };
        state.port_settings = make_default_port_settings();
        state.port_vlan = make_default_port_vlan();
        state.port_pvids = make_default_port_pvids();
        state.igmp_config = IgmpConfig {
            igmp: "on".to_string(),
            fast_leave: "off".to_string(),
            report_flood: "off".to_string(),
        };
        state.storm_control.ports.iter_mut().for_each(|p| {
            p.sctrl_bcast = 0;
            p.sctrl_mcast = 0;
            p.sctrl_unucast = 0;
            p.sctrl_unmcast = 0;
        });
        state.port_mirror.monitoring_port_id = "0".to_string();
        state.port_mirror.port_1 = mirror_disabled(1);
        state.port_mirror.port_2 = mirror_disabled(2);
        state.port_mirror.port_3 = mirror_disabled(3);
        state.port_mirror.port_4 = mirror_disabled(4);
        state.port_mirror.port_5 = mirror_disabled(5);
        state.port_mirror.port_6 = mirror_disabled(6);
        state.port_mirror.port_7 = mirror_disabled(7);
        state.port_mirror.port_8 = mirror_disabled(8);
        state.port_mirror.port_9 = mirror_disabled(9);
        state.port_mirror.port_10 = mirror_disabled(10);
        state.stp_config = StpConfig {
            stp_enable: "0".to_string(),
            stp_rstp_mode: "RSTP".to_string(),
            num_ports: "10".to_string(),
            raw: serde_json::json!({}),
        };
        state.trunk_config = TrunkConfigResponse {
            port_num: 10,
            system_priority: 32768,
            raw: serde_json::json!({}),
        };
        state.dynamic_mac_entries.clear();
        state.static_mac_entries.clear();

        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    /// `GET /config/download` — returns JSON snapshot of current state
    pub fn backup_config(&self) -> Result<String> {
        self.check_session()?;
        let state = self.state.borrow();
        let snapshot = SwitchConfigSnapshot {
            metadata: ConfigMetadata {
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                switch_host: self.host.clone(),
                switch_name: self.host.clone(),
                timestamp: chrono::Local::now().to_rfc3339(),
            },
            system_info: state.system_info.clone(),
            network_settings: state.network_settings.clone(),
            port_settings: state.port_settings.clone(),
            port_vlan: state.port_vlan.clone(),
            igmp_config: state.igmp_config.clone(),
            storm_control: state.storm_control.clone(),
            port_mirror: state.port_mirror.clone(),
            stp_config: state.stp_config.clone(),
            trunk_config: state.trunk_config.clone(),
        };
        serde_json::to_string_pretty(&snapshot).map_err(|e| anyhow::anyhow!(e))
    }

    /// `POST /config/upload` — restores from JSON snapshot
    pub fn restore_config(&self, config_json: &str) -> Result<WriteResponse> {
        self.check_session()?;
        let snapshot: SwitchConfigSnapshot =
            serde_json::from_str(config_json).map_err(|e| anyhow::anyhow!(e))?;

        self.log_write("restore_config", config_json);

        let mut state = self.state.borrow_mut();
        state.system_info.des = snapshot.system_info.des;
        state.network_settings = snapshot.network_settings;
        state.port_settings = snapshot.port_settings;
        state.port_vlan = snapshot.port_vlan;
        state.igmp_config = snapshot.igmp_config;
        state.storm_control = snapshot.storm_control;
        state.port_mirror = snapshot.port_mirror;
        state.stp_config = snapshot.stp_config;
        state.trunk_config = snapshot.trunk_config;

        Ok(WriteResponse {
            result: Some("success".to_string()),
        })
    }

    // -----------------------------------------------------------------------
    // Test helpers
    // -----------------------------------------------------------------------

    /// Return the hostname this mock is pretending to be.
    #[allow(dead_code)]
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Return a clone of the write log for test assertions.
    #[allow(dead_code)]
    pub fn write_log(&self) -> Vec<MockWriteEntry> {
        self.state.borrow().write_log.clone()
    }

    /// Clear the write log (useful between test phases).
    #[allow(dead_code)]
    pub fn clear_log(&self) {
        self.state.borrow_mut().write_log.clear();
    }

    // -----------------------------------------------------------------------
    // Internal helpers
    // -----------------------------------------------------------------------

    fn check_session(&self) -> Result<()> {
        if !self.state.borrow().authenticated {
            bail!("Mock: session expired — please reconnect");
        }
        Ok(())
    }

    fn log_write(&self, operation: &str, payload: &str) {
        self.state.borrow_mut().write_log.push(MockWriteEntry {
            operation: operation.to_string(),
            payload: payload.to_string(),
        });
    }
}

// ---------------------------------------------------------------------------
// Default state builders
// ---------------------------------------------------------------------------

fn make_default_port_settings() -> PortSettingsResponse {
    let mk = |id: u32| PortCfg {
        eee_status: "eee_inactive".to_string(),
        port_id: id.to_string(),
        port_status: "Enabled".to_string(),
        spd_duplex_cfg: "Auto".to_string(),
        spd_duplex_actual: "Link Down".to_string(),
        flow_ctrl_cfg: "On".to_string(),
        flow_ctrl_actual: "On".to_string(),
    };
    PortSettingsResponse {
        port_num: "10".to_string(),
        port_mode: "PORT_MODE_8_PLUS_2".to_string(),
        port_1: mk(1),
        port_2: mk(2),
        port_3: mk(3),
        port_4: mk(4),
        port_5: mk(5),
        port_6: mk(6),
        port_7: mk(7),
        port_8: mk(8),
        port_9: mk(9),
        port_10: mk(10),
    }
}

fn make_default_port_statistics() -> PortStatisticsResponse {
    let mk = |id: u32| PortStats {
        port_id: id.to_string(),
        port_status: "Enabled".to_string(),
        link_status: "Link Down".to_string(),
        tx_good_pkt: "0".to_string(),
        tx_bad_pkt: "0".to_string(),
        rx_good_pkt: "0".to_string(),
        rx_bad_pkt: "0".to_string(),
    };
    PortStatisticsResponse {
        port_num: "10".to_string(),
        port_1: mk(1),
        port_2: mk(2),
        port_3: mk(3),
        port_4: mk(4),
        port_5: mk(5),
        port_6: mk(6),
        port_7: mk(7),
        port_8: mk(8),
        port_9: mk(9),
        port_10: mk(10),
    }
}

fn make_default_port_vlan() -> PortVlanResponse {
    let mk = |id: u32| PortVlanEntry {
        port_id: id,
        pvid: 1,
        frame_type: 0,
    };
    PortVlanResponse {
        port_num: 10,
        port_1: mk(1),
        port_2: mk(2),
        port_3: mk(3),
        port_4: mk(4),
        port_5: mk(5),
        port_6: mk(6),
        port_7: mk(7),
        port_8: mk(8),
        port_9: mk(9),
        port_10: mk(10),
    }
}

fn make_default_port_pvids() -> PortPvidsResponse {
    PortPvidsResponse {
        port_pvids: vec![0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
    }
}

fn mirror_disabled(id: u32) -> PortMirrorEntry {
    PortMirrorEntry {
        port_id: id.to_string(),
        ingress_status: "Disabled".to_string(),
        egress_status: "Disabled".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Mutation helpers (avoid repetitive per-port code)
// ---------------------------------------------------------------------------

fn apply_port_setting_to_cfg(cfg: &mut PortCfg, update: &Option<PortSettingsRequest>) {
    if let Some(u) = update {
        if let Some(ref v) = u.port_status {
            cfg.port_status = v.clone();
        }
        if let Some(ref v) = u.spd_duplex_cfg {
            cfg.spd_duplex_cfg = v.clone();
        }
        if let Some(ref v) = u.flow_ctrl_cfg {
            cfg.flow_ctrl_cfg = v.clone();
            cfg.flow_ctrl_actual = v.clone(); // mock: config = actual
        }
    }
}

fn apply_vlan_to_port(entry: &mut PortVlanEntry, update: &Option<VlanPortEntry>) {
    if let Some(u) = update {
        entry.pvid = u.pvid;
        entry.frame_type = u.frame_type;
    }
}

fn update_pvid_array(pvids: &mut PortPvidsResponse, update: &Option<VlanPortEntry>) {
    if let Some(u) = update {
        let idx = u.port_id as usize;
        if idx < pvids.port_pvids.len() {
            pvids.port_pvids[idx] = u.pvid;
        }
    }
}

fn set_port_stats(stats: &mut PortStatisticsResponse, port_id: u32, new: PortStats) {
    match port_id {
        1 => stats.port_1 = new,
        2 => stats.port_2 = new,
        3 => stats.port_3 = new,
        4 => stats.port_4 = new,
        5 => stats.port_5 = new,
        6 => stats.port_6 = new,
        7 => stats.port_7 = new,
        8 => stats.port_8 = new,
        9 => stats.port_9 = new,
        10 => stats.port_10 = new,
        _ => {}
    }
}

fn update_trunk_raw(raw: &mut serde_json::Value, port_id: u32, entry: &Option<TrunkPortEntry>) {
    if let Some(e) = entry {
        let key = format!("Port_{}", port_id);
        let port_obj = serde_json::json!({
            format!("portTypeId_{}", port_id): e.port_type,
            format!("portPriorityId_{}", port_id): e.port_priority,
            format!("lacpTimeoutId_{}", port_id): e.lacp_timeout,
            format!("Port_{}_grpInd", port_id): e.group_index,
            format!("Port_{}_state", port_id): e.state,
        });
        raw[key] = port_obj;
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_new_returns_defaults() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let info = client.get_system_info().unwrap();
        assert_eq!(info.des, "SKS3200-8E2X");
        assert_eq!(info.temperature, "45");
    }

    #[test]
    fn test_mock_set_description() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let resp = client.set_description("My Switch").unwrap();
        assert!(resp.is_success());
        assert_eq!(client.get_system_info().unwrap().des, "My Switch");

        let log = client.write_log();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].operation, "set_description");
    }

    #[test]
    fn test_mock_set_port_status() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = PortSettingsApplyRequest::single_port(
            1,
            PortSettingsRequest {
                port_status: Some("Disabled".to_string()),
                ..Default::default()
            },
        )
        .unwrap();
        let resp = client.set_port_settings(&req).unwrap();
        assert!(resp.is_success());

        let ports = client.get_port_settings().unwrap();
        assert_eq!(ports.port_1.port_status, "Disabled");
    }

    #[test]
    fn test_mock_set_network_settings() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = NetworkSettingsRequest {
            ip_address: Some("10.0.0.1".to_string()),
            dhcp_enabled: Some("1".to_string()),
            ..Default::default()
        };
        client.set_network_settings(&req).unwrap();

        let net = client.get_network_settings().unwrap();
        assert_eq!(net.ip_address, "10.0.0.1");
        assert_eq!(net.dhcp_enabled, "1");
    }

    #[test]
    fn test_mock_set_vlan_pvid() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = PortVlanRequest::single_port(3, 100, 1).unwrap();
        client.set_port_vlan(&req).unwrap();

        let vlan = client.get_port_vlan().unwrap();
        assert_eq!(vlan.port_3.pvid, 100);
        assert_eq!(vlan.port_3.frame_type, 1);

        // PVID array should also be updated
        let pvids = client.get_all_port_pvids().unwrap();
        assert_eq!(pvids.port_pvids[3], 100);
    }

    #[test]
    fn test_mock_clear_statistics() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        // Pre-populate some counters by modifying state directly
        {
            let mut state = client.state.borrow_mut();
            state.port_statistics.port_1.tx_good_pkt = "12345".to_string();
        }
        client.clear_statistics().unwrap();

        let stats = client.get_port_statistics().unwrap();
        assert_eq!(stats.port_1.tx_good_pkt, "0");
        assert_eq!(stats.port_1.tx_bad_pkt, "0");
    }

    #[test]
    fn test_mock_clear_mac_entries() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let entries = client.get_dynamic_mac_entries().unwrap();
        assert!(!entries.is_empty());

        client.clear_mac_entries().unwrap();
        let empty = client.get_dynamic_mac_entries().unwrap();
        assert!(empty.is_empty());
    }

    #[test]
    fn test_mock_backup_restore_roundtrip() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");

        // Modify something
        client.set_description("Changed").unwrap();

        // Backup
        let backup = client.backup_config().unwrap();
        assert!(backup.contains("Changed"));
        assert!(backup.contains("metadata"));

        // Change description again
        client.set_description("Something Else").unwrap();

        // Restore from backup
        client.restore_config(&backup).unwrap();

        // Should be back to "Changed"
        let info = client.get_system_info().unwrap();
        assert_eq!(info.des, "Changed");
    }

    #[test]
    fn test_mock_write_log_tracks_operations() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        client.set_description("test").unwrap();
        client.save_config().unwrap();

        let log = client.write_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].operation, "set_description");
        assert_eq!(log[1].operation, "save_config");
    }

    #[test]
    fn test_mock_igmp_config() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = IgmpConfigRequest {
            igmp: Some("off".to_string()),
            fast_leave: Some("on".to_string()),
            report_flood: Some("on".to_string()),
        };
        client.set_igmp_config(&req).unwrap();

        let cfg = client.get_igmp_config().unwrap();
        assert_eq!(cfg.igmp, "off");
        assert_eq!(cfg.fast_leave, "on");
        assert_eq!(cfg.report_flood, "on");
    }

    #[test]
    fn test_mock_stp_enable() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = StpConfigRequest::enable();
        client.set_stp_config(&req).unwrap();

        let stp = client.get_stp_config().unwrap();
        assert_eq!(stp.stp_enable, "1");
    }

    #[test]
    fn test_mock_port_mirror() {
        let client = MockSwitchClient::new("192.168.1.1", "admin", "admin");
        let req = PortMirrorRequest {
            port_num: "10".to_string(),
            monitoring_port_id: "5".to_string(),
            port_1: Some(PortMirrorEntry {
                port_id: "1".to_string(),
                ingress_status: "Enabled".to_string(),
                egress_status: "Disabled".to_string(),
            }),
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
        client.set_port_mirror(&req).unwrap();

        let mirror = client.get_port_mirror().unwrap();
        assert_eq!(mirror.monitoring_port_id, "5");
        assert_eq!(mirror.port_1.ingress_status, "Enabled");
    }
}
