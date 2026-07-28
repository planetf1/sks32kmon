use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use reqwest::blocking::Client as HttpClient;
use reqwest::cookie::Jar;
use std::sync::Arc;
use std::time::Duration;

use crate::models::*;
use crate::write_models::*;

/// A session-authenticated HTTP client for an SKS3200 switch.
pub struct SwitchClient {
    base_url: String,
    http: HttpClient,
}

impl SwitchClient {
    /// Create a new client and authenticate against the switch.
    ///
    /// The SKS3200 auth mechanism:
    ///   GET /authorize?loginusr=<md5(username)>&loginpwd=<md5(password)>
    /// Returns a session cookie on success.
    pub fn connect(host: &str, username: &str, password: &str) -> Result<Self> {
        let base_url = format!("http://{}", host);
        let cookie_jar = Arc::new(Jar::default());
        let http = HttpClient::builder()
            .cookie_store(true)
            .cookie_provider(cookie_jar.clone())
            .timeout(Duration::from_secs(10))
            .build()
            .context("Failed to build HTTP client")?;

        let client = Self { base_url, http };

        // Compute MD5 hashes
        let usr_hash = md5_hash(username);
        let pwd_hash = md5_hash(password);

        let url = format!(
            "{}/authorize?loginusr={}&loginpwd={}",
            client.base_url, usr_hash, pwd_hash
        );

        let resp = client
            .http
            .get(&url)
            .send()
            .context("Failed to send auth request")?;

        let text = resp.text().context("Failed to read auth response")?;

        // Success returns a redirect to index.html (or setup.html), failure redirects to login.html
        if text.contains("login.html") {
            bail!("Authentication failed for {} — check credentials", host);
        }

        Ok(client)
    }

    // -- Read-only API calls ------------------------------------------------

    /// System Information — `GET /status.json`
    pub fn get_system_info(&self) -> Result<SystemInfo> {
        self.get_json("status.json")
    }

    /// Network Settings — `GET /network_settings.json`
    pub fn get_network_settings(&self) -> Result<NetworkSettings> {
        self.get_json("network_settings.json")
    }

    /// Port Settings — `GET /port_setting_load.json`
    pub fn get_port_settings(&self) -> Result<PortSettingsResponse> {
        self.get_json("port_setting_load.json")
    }

    /// Port Statistics — `GET /port_statistics.json`
    pub fn get_port_statistics(&self) -> Result<PortStatisticsResponse> {
        self.get_json("port_statistics.json")
    }

    /// Dynamic MAC Table — `GET /mac_get_dynamic_mac_entries.json`
    ///
    /// Note: The API returns `data: [{...}, {...}]` lines (JSON with `data:` prefix).
    pub fn get_dynamic_mac_entries(&self) -> Result<Vec<MacEntry>> {
        let raw = self.get_raw("mac_get_dynamic_mac_entries.json")?;
        parse_mac_entries(&raw)
    }

    /// Static MAC Table — `GET /mac_get_static_mac_entries.json`
    pub fn get_static_mac_entries(&self) -> Result<Vec<StaticMacEntry>> {
        let raw = self.get_raw("mac_get_static_mac_entries.json")?;
        parse_static_mac_entries(&raw)
    }

    /// Loop Detection Status — `GET /port_loop_status.json`
    pub fn get_loop_status(&self) -> Result<LoopStatusResponse> {
        self.get_json("port_loop_status.json")
    }

    /// STP Config — `GET /stp.json`
    pub fn get_stp_config(&self) -> Result<StpConfig> {
        self.get_json("stp.json")
    }

    /// Port VLAN Config — `GET /port_vlan.json`
    pub fn get_port_vlan(&self) -> Result<PortVlanResponse> {
        self.get_json("port_vlan.json")
    }

    /// All Port PVIDs — `GET /all_port_pvid.json`
    pub fn get_all_port_pvids(&self) -> Result<PortPvidsResponse> {
        self.get_json("all_port_pvid.json")
    }

    /// IGMP Config — `GET /igmp_config.json`
    pub fn get_igmp_config(&self) -> Result<IgmpConfig> {
        self.get_json("igmp_config.json")
    }

    /// Storm Control Config — `GET /storm_ctrl_cfg.json`
    pub fn get_storm_control(&self) -> Result<StormControlResponse> {
        self.get_json("storm_ctrl_cfg.json")
    }

    /// Port Mirror Config — `GET /port_mirror.json`
    pub fn get_port_mirror(&self) -> Result<PortMirrorResponse> {
        self.get_json("port_mirror.json")
    }

    /// Link Aggregation Config — `GET /port_trunk_cfg.json`
    pub fn get_trunk_config(&self) -> Result<TrunkConfigResponse> {
        self.get_json("port_trunk_cfg.json")
    }

    // -- Write API calls ----------------------------------------------------

    /// Set device description — `POST /set_des.json`
    /// **Confidence: certain** (captured from web UI)
    pub fn set_description(&self, description: &str) -> Result<WriteResponse> {
        let body = serde_json::json!({"des": description});
        self.post_json("set_des.json", &body)
    }

    /// Apply port settings — `POST /apply_user_port_setting.json`
    /// **Confidence: likely** (inferred from GET response shape)
    pub fn set_port_settings(&self, request: &PortSettingsApplyRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("apply_user_port_setting.json", &body)
    }

    /// Update IPv4 network settings — `POST /network_settings_ipv4.json`
    /// **Confidence: likely** (inferred from GET response)
    pub fn set_network_settings(&self, request: &NetworkSettingsRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("network_settings_ipv4.json", &body)
    }

    /// Set per-port VLAN config — `POST /port_vlan.json`
    /// **Confidence: speculative**
    pub fn set_port_vlan(&self, request: &PortVlanRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("port_vlan.json", &body)
    }

    /// Update IGMP snooping config — `POST /igmp_config.json`
    /// **Confidence: speculative**
    pub fn set_igmp_config(&self, request: &IgmpConfigRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("igmp_config.json", &body)
    }

    /// Set storm control rates — `POST /storm_ctrl_cfg.json`
    /// **Confidence: speculative**
    pub fn set_storm_control(&self, request: &StormControlRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("storm_ctrl_cfg.json", &body)
    }

    /// Configure port mirroring — `POST /port_mirror.json`
    /// **Confidence: speculative**
    pub fn set_port_mirror(&self, request: &PortMirrorRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("port_mirror.json", &body)
    }

    /// Update STP configuration — `POST /stp.json`
    /// **Confidence: speculative**
    pub fn set_stp_config(&self, request: &StpConfigRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("stp.json", &body)
    }

    /// Set trunk/LACP config — `POST /port_trunk_cfg.json`
    /// **Confidence: speculative**
    pub fn set_trunk_config(&self, request: &TrunkConfigRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("port_trunk_cfg.json", &body)
    }

    /// Configure loop protection — `POST /port_lock_cfg.json`
    pub fn set_loop_protection(&self, request: &LoopProtectionRequest) -> Result<WriteResponse> {
        let body = serde_json::to_value(request)?;
        self.post_json("port_lock_cfg.json", &body)
    }

    /// Save running config to startup — `POST /save_all_configs.json`
    pub fn save_config(&self) -> Result<WriteResponse> {
        let body = serde_json::json!({});
        self.post_json("save_all_configs.json", &body)
    }

    /// Clear port statistics — `GET /clear_statistics.json`
    /// Note: this endpoint uses GET, not POST (switch bug/quirk).
    pub fn clear_statistics(&self) -> Result<WriteResponse> {
        let url = self.url("clear_statistics.json");
        let resp = self
            .http
            .get(&url)
            .send()
            .context("GET clear_statistics.json failed")?;

        if !resp.status().is_success() {
            bail!("GET clear_statistics.json returned HTTP {}", resp.status());
        }

        let body = resp.text().context("Read clear_statistics.json failed")?;

        if body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        serde_json::from_str(&body).context("Failed to parse clear_statistics.json response")
    }

    /// Clear dynamic MAC table — `POST /mac_clear_dynamic_mac_entries.json`
    pub fn clear_mac_entries(&self) -> Result<WriteResponse> {
        let body = serde_json::json!({});
        self.post_json("mac_clear_dynamic_mac_entries.json", &body)
    }

    /// Reboot the switch — `POST /system_reboot.json`
    /// WARNING: Destructive operation. Switch becomes unreachable for ~30s.
    pub fn reboot(&self) -> Result<WriteResponse> {
        let body = serde_json::json!({});
        self.post_json("system_reboot.json", &body)
    }

    /// Factory reset the switch — `POST /factory_reset.json`
    /// WARNING: Destroys all configuration. Requires reconfiguration.
    pub fn factory_reset(&self) -> Result<WriteResponse> {
        let body = serde_json::json!({});
        self.post_json("factory_reset.json", &body)
    }

    /// Return the host this client is connected to (without protocol).
    pub fn host(&self) -> &str {
        self.base_url
            .strip_prefix("http://")
            .unwrap_or(&self.base_url)
    }

    /// Download config backup — reads all endpoints and serializes to JSON.
    pub fn backup_config(&self) -> Result<String> {
        let snapshot = SwitchConfigSnapshot {
            metadata: ConfigMetadata {
                tool_version: env!("CARGO_PKG_VERSION").to_string(),
                switch_host: self.host().to_string(),
                switch_name: self.host().to_string(),
                timestamp: chrono::Local::now().to_rfc3339(),
            },
            system_info: self.get_system_info()?,
            network_settings: self.get_network_settings()?,
            port_settings: self.get_port_settings()?,
            port_vlan: self.get_port_vlan()?,
            igmp_config: self.get_igmp_config()?,
            storm_control: self.get_storm_control()?,
            port_mirror: self.get_port_mirror()?,
            stp_config: self.get_stp_config()?,
            trunk_config: self.get_trunk_config()?,
        };
        serde_json::to_string_pretty(&snapshot).context("Failed to serialize config snapshot")
    }

    /// Restore config from JSON snapshot.
    ///
    /// Applies sections in safe order: description, ports, VLAN, IGMP, storm,
    /// mirror, STP, trunk → save → network settings LAST (since IP change
    /// disconnects the client).
    ///
    /// On real switches: network settings are applied but may cause a disconnect
    /// before the response arrives. The caller should handle this.
    pub fn restore_config(&self, config_json: &str) -> Result<WriteResponse> {
        let snapshot: SwitchConfigSnapshot =
            serde_json::from_str(config_json).context("Failed to parse config snapshot")?;

        // 1. Description (lowest risk)
        self.set_description(&snapshot.system_info.des)?;

        // 2. Port settings
        let port_req = port_settings_response_to_apply(&snapshot.port_settings);
        self.set_port_settings(&port_req)?;

        // 3. VLAN
        let vlan_req = port_vlan_response_to_request(&snapshot.port_vlan);
        self.set_port_vlan(&vlan_req)?;

        // 4. IGMP
        let igmp = &snapshot.igmp_config;
        self.set_igmp_config(&IgmpConfigRequest {
            igmp: Some(igmp.igmp.clone()),
            fast_leave: Some(igmp.fast_leave.clone()),
            report_flood: Some(igmp.report_flood.clone()),
        })?;

        // 5. Storm control
        self.set_storm_control(&StormControlRequest {
            portnum: snapshot.storm_control.portnum,
            ports: snapshot
                .storm_control
                .ports
                .iter()
                .map(|p| StormControlPortRequest {
                    port_id: p.port_id,
                    sctrl_bcast: p.sctrl_bcast,
                    sctrl_mcast: p.sctrl_mcast,
                    sctrl_unucast: p.sctrl_unucast,
                    sctrl_unmcast: p.sctrl_unmcast,
                })
                .collect(),
        })?;

        // 6. Port mirror
        self.set_port_mirror(&port_mirror_response_to_request(&snapshot.port_mirror))?;

        // 7. STP
        self.set_stp_config(&StpConfigRequest {
            stp_enable: Some(snapshot.stp_config.stp_enable.clone()),
            stp_rstp_mode: Some(snapshot.stp_config.stp_rstp_mode.clone()),
            num_ports: Some(snapshot.stp_config.num_ports.clone()),
            raw: snapshot.stp_config.raw.clone(),
        })?;

        // 8. Save everything applied so far
        self.save_config()?;

        // 9. Network settings LAST — may cause disconnect on IP change
        let net = &snapshot.network_settings;
        self.set_network_settings(&NetworkSettingsRequest {
            ip_address: Some(net.ip_address.clone()),
            netmask: Some(net.netmask.clone()),
            gateway: Some(net.gateway.clone()),
            dhcp_enabled: Some(net.dhcp_enabled.clone()),
            dns_server: Some(net.dns_server.clone()),
            auto_dns_enabled: Some(net.auto_dns_enabled.clone()),
        })?;

        // 10. Save network settings too
        self.save_config()
    }

    // -- Internal helpers (continued) ---------------------------------------

    fn url(&self, endpoint: &str) -> String {
        format!("{}/{}", self.base_url, endpoint)
    }

    /// Fetch an endpoint and deserialize as JSON.
    fn get_json<T: serde::de::DeserializeOwned>(&self, endpoint: &str) -> Result<T> {
        let url = self.url(endpoint);
        let resp = self
            .http
            .get(&url)
            .send()
            .with_context(|| format!("GET {} failed", endpoint))?;

        if !resp.status().is_success() {
            bail!("GET {} returned HTTP {}", endpoint, resp.status());
        }

        let body = resp
            .text()
            .with_context(|| format!("Read {} failed", endpoint))?;

        // Check for session expiry
        if body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        serde_json::from_str(&body)
            .with_context(|| format!("Failed to parse JSON from {}", endpoint))
    }

    /// Fetch an endpoint and return raw text (for endpoints with non-standard JSON).
    fn get_raw(&self, endpoint: &str) -> Result<String> {
        let url = self.url(endpoint);
        let resp = self
            .http
            .get(&url)
            .send()
            .with_context(|| format!("GET {} failed", endpoint))?;

        if !resp.status().is_success() {
            bail!("GET {} returned HTTP {}", endpoint, resp.status());
        }

        let body = resp
            .text()
            .with_context(|| format!("Read {} failed", endpoint))?;

        if body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        Ok(body)
    }

    /// POST an endpoint with a JSON body and deserialize the response.
    ///
    /// Mirrors `get_json` but uses POST with `Content-Type: application/json`.
    /// Checks for session expiry in the response body.
    fn post_json<T: serde::Serialize>(&self, endpoint: &str, body: &T) -> Result<WriteResponse> {
        let url = self.url(endpoint);
        let resp = self
            .http
            .post(&url)
            .json(body)
            .send()
            .with_context(|| format!("POST {} failed", endpoint))?;

        if !resp.status().is_success() {
            bail!("POST {} returned HTTP {}", endpoint, resp.status());
        }

        let response_body = resp
            .text()
            .with_context(|| format!("Read {} response failed", endpoint))?;

        // Check for session expiry
        if response_body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        // Try to parse as WriteResponse, but accept empty/redirect responses too
        if response_body.trim().is_empty() {
            return Ok(WriteResponse {
                result: Some("success".to_string()),
            });
        }

        serde_json::from_str(&response_body)
            .with_context(|| format!("Failed to parse JSON from {}", endpoint))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

pub fn md5_hash(input: &str) -> String {
    let mut hasher = Md5::new();
    hasher.update(input.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Parse dynamic MAC entries from the weird `data: [...]data: [...]` format.
pub fn parse_mac_entries(raw: &str) -> Result<Vec<MacEntry>> {
    let mut all = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if let Some(json) = line.strip_prefix("data: ") {
            let batch: Vec<MacEntry> =
                serde_json::from_str(json).context("Failed to parse MAC entry batch")?;
            all.extend(batch);
        }
    }
    Ok(all)
}

/// Parse static MAC entries (same format).
pub fn parse_static_mac_entries(raw: &str) -> Result<Vec<StaticMacEntry>> {
    let mut all = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if let Some(json) = line.strip_prefix("data: ") {
            let batch: Vec<StaticMacEntry> =
                serde_json::from_str(json).context("Failed to parse static MAC entry batch")?;
            all.extend(batch);
        }
    }
    Ok(all)
}

// ---------------------------------------------------------------------------
// Restore helpers — convert read response types to write request types
// ---------------------------------------------------------------------------

/// Build a `PortSettingsApplyRequest` from a `PortSettingsResponse`.
fn port_settings_response_to_apply(resp: &PortSettingsResponse) -> PortSettingsApplyRequest {
    fn to_req(cfg: &PortCfg) -> PortSettingsRequest {
        PortSettingsRequest {
            port_status: Some(cfg.port_status.clone()),
            spd_duplex_cfg: Some(cfg.spd_duplex_cfg.clone()),
            flow_ctrl_cfg: Some(cfg.flow_ctrl_cfg.clone()),
        }
    }

    PortSettingsApplyRequest {
        port_num: Some(resp.port_num.clone()),
        port_mode: Some(resp.port_mode.clone()),
        port_1: Some(to_req(&resp.port_1)),
        port_2: Some(to_req(&resp.port_2)),
        port_3: Some(to_req(&resp.port_3)),
        port_4: Some(to_req(&resp.port_4)),
        port_5: Some(to_req(&resp.port_5)),
        port_6: Some(to_req(&resp.port_6)),
        port_7: Some(to_req(&resp.port_7)),
        port_8: Some(to_req(&resp.port_8)),
        port_9: Some(to_req(&resp.port_9)),
        port_10: Some(to_req(&resp.port_10)),
    }
}

/// Build a `PortVlanRequest` from a `PortVlanResponse`.
fn port_vlan_response_to_request(resp: &PortVlanResponse) -> PortVlanRequest {
    fn to_entry(e: &PortVlanEntry) -> Option<VlanPortEntry> {
        Some(VlanPortEntry {
            port_id: e.port_id,
            pvid: e.pvid,
            frame_type: e.frame_type,
        })
    }

    PortVlanRequest {
        port_num: resp.port_num,
        port_1: to_entry(&resp.port_1),
        port_2: to_entry(&resp.port_2),
        port_3: to_entry(&resp.port_3),
        port_4: to_entry(&resp.port_4),
        port_5: to_entry(&resp.port_5),
        port_6: to_entry(&resp.port_6),
        port_7: to_entry(&resp.port_7),
        port_8: to_entry(&resp.port_8),
        port_9: to_entry(&resp.port_9),
        port_10: to_entry(&resp.port_10),
    }
}

/// Build a `PortMirrorRequest` from a `PortMirrorResponse`.
fn port_mirror_response_to_request(resp: &PortMirrorResponse) -> PortMirrorRequest {
    fn to_entry(e: &PortMirrorEntry) -> Option<PortMirrorEntry> {
        Some(PortMirrorEntry {
            port_id: e.port_id.clone(),
            ingress_status: e.ingress_status.clone(),
            egress_status: e.egress_status.clone(),
        })
    }

    PortMirrorRequest {
        port_num: resp.port_num.clone(),
        monitoring_port_id: resp.monitoring_port_id.clone(),
        port_1: to_entry(&resp.port_1),
        port_2: to_entry(&resp.port_2),
        port_3: to_entry(&resp.port_3),
        port_4: to_entry(&resp.port_4),
        port_5: to_entry(&resp.port_5),
        port_6: to_entry(&resp.port_6),
        port_7: to_entry(&resp.port_7),
        port_8: to_entry(&resp.port_8),
        port_9: to_entry(&resp.port_9),
        port_10: to_entry(&resp.port_10),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_md5_hash() {
        assert_eq!(md5_hash("admin"), "21232f297a57a5a743894a0e4a801fc3");
    }

    #[test]
    fn test_parse_mac_entries() {
        let raw = "data: [{\"Dynamic_idx\":1,\"Dynamic_mac_addr\":\"00:0E:58:85:04:82\",\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":8,\"Dynamic_age_timer\":244}]\n";
        let entries = parse_mac_entries(raw).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].mac_addr, "00:0E:58:85:04:82");
        assert_eq!(entries[0].port_id, 8);
        assert_eq!(entries[0].age_timer, 244);
    }

    #[test]
    fn test_parse_mac_entries_multiple_lines() {
        let raw = concat!(
            "data: [{\"Dynamic_idx\":1,\"Dynamic_mac_addr\":\"00:0E:58:85:04:82\",\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":8,\"Dynamic_age_timer\":244}]\n",
            "data: [{\"Dynamic_idx\":2,\"Dynamic_mac_addr\":\"DC:A6:32:43:C4:B0\",\"Dynamic_vlan_id\":1,\"Dynamic_fid\":0,\"Dynamic_portid\":6,\"Dynamic_age_timer\":244}]\n",
        );
        let entries = parse_mac_entries(raw).unwrap();
        assert_eq!(entries.len(), 2);
    }
}
