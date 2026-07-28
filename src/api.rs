//! Dispatch enum that wraps either a real or mock switch client.
//!
//! All CLI/TUI code operates on `ApiClient` without knowing the
//! underlying implementation. Methods delegate via a macro to
//! whichever variant is active.
//!
//! # Default behaviour
//!
//! - **Reads** always go to the real switch (`connect_real`).
//! - **Writes** default to mock (`connect_mock`) for safety.
//! - Use `--live` to route writes to the real switch.

use anyhow::Result;

use crate::client::SwitchClient;
use crate::mock::MockSwitchClient;
use crate::models::*;
use crate::write_models::*;

/// A switch client that is either real (HTTP) or mock (in-memory).
pub enum ApiClient {
    Real(SwitchClient),
    Mock(Box<MockSwitchClient>),
}

impl ApiClient {
    /// Create a client connected to a real switch.
    pub fn connect_real(host: &str, username: &str, password: &str) -> Result<Self> {
        let client = SwitchClient::connect(host, username, password)?;
        Ok(ApiClient::Real(client))
    }

    /// Create a mock client with in-memory state.
    pub fn connect_mock(host: &str, username: &str, password: &str) -> Result<Self> {
        MockSwitchClient::verify_auth(username, password)?;
        Ok(ApiClient::Mock(Box::new(MockSwitchClient::new(
            host, username, password,
        ))))
    }

    /// Is this client operating against a real switch?
    pub fn is_real(&self) -> bool {
        matches!(self, ApiClient::Real(_))
    }

    /// Is this client in mock mode?
    pub fn is_mock(&self) -> bool {
        matches!(self, ApiClient::Mock(_))
    }

    /// Return the host this client is connected to.
    #[allow(dead_code)]
    pub fn host(&self) -> &str {
        match self {
            ApiClient::Real(c) => c.host(),
            ApiClient::Mock(c) => c.host(),
        }
    }

    // =======================================================================
    // Read API — delegates to inner client
    // =======================================================================

    pub fn get_system_info(&self) -> Result<SystemInfo> {
        match self {
            ApiClient::Real(c) => c.get_system_info(),
            ApiClient::Mock(c) => c.get_system_info(),
        }
    }

    pub fn get_network_settings(&self) -> Result<NetworkSettings> {
        match self {
            ApiClient::Real(c) => c.get_network_settings(),
            ApiClient::Mock(c) => c.get_network_settings(),
        }
    }

    pub fn get_port_settings(&self) -> Result<PortSettingsResponse> {
        match self {
            ApiClient::Real(c) => c.get_port_settings(),
            ApiClient::Mock(c) => c.get_port_settings(),
        }
    }

    pub fn get_port_statistics(&self) -> Result<PortStatisticsResponse> {
        match self {
            ApiClient::Real(c) => c.get_port_statistics(),
            ApiClient::Mock(c) => c.get_port_statistics(),
        }
    }

    pub fn get_dynamic_mac_entries(&self) -> Result<Vec<MacEntry>> {
        match self {
            ApiClient::Real(c) => c.get_dynamic_mac_entries(),
            ApiClient::Mock(c) => c.get_dynamic_mac_entries(),
        }
    }

    pub fn get_static_mac_entries(&self) -> Result<Vec<StaticMacEntry>> {
        match self {
            ApiClient::Real(c) => c.get_static_mac_entries(),
            ApiClient::Mock(c) => c.get_static_mac_entries(),
        }
    }

    pub fn get_loop_status(&self) -> Result<LoopStatusResponse> {
        match self {
            ApiClient::Real(c) => c.get_loop_status(),
            ApiClient::Mock(c) => c.get_loop_status(),
        }
    }

    pub fn get_stp_config(&self) -> Result<StpConfig> {
        match self {
            ApiClient::Real(c) => c.get_stp_config(),
            ApiClient::Mock(c) => c.get_stp_config(),
        }
    }

    pub fn get_port_vlan(&self) -> Result<PortVlanResponse> {
        match self {
            ApiClient::Real(c) => c.get_port_vlan(),
            ApiClient::Mock(c) => c.get_port_vlan(),
        }
    }

    pub fn get_all_port_pvids(&self) -> Result<PortPvidsResponse> {
        match self {
            ApiClient::Real(c) => c.get_all_port_pvids(),
            ApiClient::Mock(c) => c.get_all_port_pvids(),
        }
    }

    pub fn get_igmp_config(&self) -> Result<IgmpConfig> {
        match self {
            ApiClient::Real(c) => c.get_igmp_config(),
            ApiClient::Mock(c) => c.get_igmp_config(),
        }
    }

    pub fn get_storm_control(&self) -> Result<StormControlResponse> {
        match self {
            ApiClient::Real(c) => c.get_storm_control(),
            ApiClient::Mock(c) => c.get_storm_control(),
        }
    }

    pub fn get_port_mirror(&self) -> Result<PortMirrorResponse> {
        match self {
            ApiClient::Real(c) => c.get_port_mirror(),
            ApiClient::Mock(c) => c.get_port_mirror(),
        }
    }

    pub fn get_trunk_config(&self) -> Result<TrunkConfigResponse> {
        match self {
            ApiClient::Real(c) => c.get_trunk_config(),
            ApiClient::Mock(c) => c.get_trunk_config(),
        }
    }

    // =======================================================================
    // Write API — delegates to inner client
    // =======================================================================

    /// `POST /set_des.json` — set device description
    pub fn set_description(&self, description: &str) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_description(description),
            ApiClient::Mock(c) => c.set_description(description),
        }
    }

    /// `POST /apply_user_port_setting.json` — apply port settings
    pub fn set_port_settings(&self, request: &PortSettingsApplyRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_port_settings(request),
            ApiClient::Mock(c) => c.set_port_settings(request),
        }
    }

    /// `POST /network_settings_ipv4.json` — update IPv4 settings
    pub fn set_network_settings(&self, request: &NetworkSettingsRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_network_settings(request),
            ApiClient::Mock(c) => c.set_network_settings(request),
        }
    }

    /// `POST /port_vlan.json` — set per-port VLAN config
    pub fn set_port_vlan(&self, request: &PortVlanRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_port_vlan(request),
            ApiClient::Mock(c) => c.set_port_vlan(request),
        }
    }

    /// `POST /igmp_config.json` — update IGMP snooping
    pub fn set_igmp_config(&self, request: &IgmpConfigRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_igmp_config(request),
            ApiClient::Mock(c) => c.set_igmp_config(request),
        }
    }

    /// `POST /storm_ctrl_cfg.json` — set storm control rates
    pub fn set_storm_control(&self, request: &StormControlRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_storm_control(request),
            ApiClient::Mock(c) => c.set_storm_control(request),
        }
    }

    /// `POST /port_mirror.json` — configure port mirroring
    pub fn set_port_mirror(&self, request: &PortMirrorRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_port_mirror(request),
            ApiClient::Mock(c) => c.set_port_mirror(request),
        }
    }

    /// `POST /stp.json` — update STP configuration
    pub fn set_stp_config(&self, request: &StpConfigRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_stp_config(request),
            ApiClient::Mock(c) => c.set_stp_config(request),
        }
    }

    /// `POST /port_trunk_cfg.json` — set trunk/LACP config
    pub fn set_trunk_config(&self, request: &TrunkConfigRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_trunk_config(request),
            ApiClient::Mock(c) => c.set_trunk_config(request),
        }
    }

    /// `POST /port_lock_cfg.json` — configure loop protection
    pub fn set_loop_protection(&self, request: &LoopProtectionRequest) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.set_loop_protection(request),
            ApiClient::Mock(c) => c.set_loop_protection(request),
        }
    }

    /// `POST /save_all_configs.json` — save running config to startup
    pub fn save_config(&self) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.save_config(),
            ApiClient::Mock(c) => c.save_config(),
        }
    }

    /// `GET /clear_statistics.json` — clear port counters
    pub fn clear_statistics(&self) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.clear_statistics(),
            ApiClient::Mock(c) => c.clear_statistics(),
        }
    }

    /// `POST /mac_clear_dynamic_mac_entries.json` — clear MAC table
    pub fn clear_mac_entries(&self) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.clear_mac_entries(),
            ApiClient::Mock(c) => c.clear_mac_entries(),
        }
    }

    /// `POST /system_reboot.json` — reboot switch
    pub fn reboot(&self) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.reboot(),
            ApiClient::Mock(c) => c.reboot(),
        }
    }

    /// `POST /factory_reset.json` — factory reset switch
    pub fn factory_reset(&self) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.factory_reset(),
            ApiClient::Mock(c) => c.factory_reset(),
        }
    }

    /// `GET /config/download` — backup config as JSON string
    pub fn backup_config(&self) -> Result<String> {
        match self {
            ApiClient::Real(c) => c.backup_config(),
            ApiClient::Mock(c) => c.backup_config(),
        }
    }

    /// `POST /config/upload` — restore config from JSON string
    pub fn restore_config(&self, config_json: &str) -> Result<WriteResponse> {
        match self {
            ApiClient::Real(c) => c.restore_config(config_json),
            ApiClient::Mock(c) => c.restore_config(config_json),
        }
    }

    // -----------------------------------------------------------------------
    // Test inspection (mock only)
    // -----------------------------------------------------------------------

    /// Return the mock write log. Returns empty vec for real client.
    #[allow(dead_code)]
    pub fn write_log(&self) -> Vec<crate::mock::MockWriteEntry> {
        match self {
            ApiClient::Mock(c) => c.write_log(),
            ApiClient::Real(_) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_client_mock_reads() {
        let client = ApiClient::connect_mock("192.168.1.1", "admin", "password123").unwrap();
        assert!(client.is_mock());
        let info = client.get_system_info().unwrap();
        assert_eq!(info.des, "SKS3200-8E2X");
    }

    #[test]
    fn test_api_client_mock_write_then_read() {
        let client = ApiClient::connect_mock("192.168.1.1", "admin", "password123").unwrap();
        client.set_description("Test Switch").unwrap();
        let info = client.get_system_info().unwrap();
        assert_eq!(info.des, "Test Switch");
    }

    #[test]
    fn test_api_client_host() {
        let client = ApiClient::connect_mock("10.0.0.50", "admin", "admin").unwrap();
        assert_eq!(client.host(), "10.0.0.50");
    }

    #[test]
    fn test_api_client_is_real() {
        let client = ApiClient::connect_mock("192.168.1.1", "admin", "admin").unwrap();
        assert!(!client.is_real());
        assert!(client.is_mock());
    }

    #[test]
    fn test_api_client_write_log_captures() {
        let client = ApiClient::connect_mock("192.168.1.1", "admin", "admin").unwrap();
        client.set_description("test").unwrap();
        client.save_config().unwrap();
        let log = client.write_log();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].operation, "set_description");
    }

    #[test]
    fn test_api_client_backup_restore_roundtrip() {
        let client = ApiClient::connect_mock("192.168.1.1", "admin", "admin").unwrap();
        client.set_description("Original").unwrap();
        let backup = client.backup_config().unwrap();

        client.set_description("Changed").unwrap();
        client.restore_config(&backup).unwrap();

        let info = client.get_system_info().unwrap();
        assert_eq!(info.des, "Original");
    }
}
