use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use reqwest::blocking::Client as HttpClient;
use reqwest::cookie::Jar;
use std::sync::Arc;
use std::time::Duration;

use crate::models::*;
use crate::v1::{self, FirmwareFamily};
use crate::write_models::*;

/// A session-authenticated HTTP client for an SKS3200-family switch.
///
/// Supports two firmware families, auto-detected at connect time from
/// `status.json`'s `fw_ver`: V2 (XikeStor SKS3200-8E2X, the tool's original
/// target) and V1 (GoodTop ZX310S-8T2XS and similar — same hardware family,
/// different/earlier firmware and web API). See `src/v1.rs` for the V1 wire
/// format and conversions.
pub struct SwitchClient {
    base_url: String,
    http: HttpClient,
    family: FirmwareFamily,
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

        let mut client = Self {
            base_url,
            http,
            family: FirmwareFamily::V2,
        };

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

        // Firmware family detection: status.json's shape is identical between
        // V1 and V2, so this always succeeds regardless of family.
        let info: SystemInfo = client.get_json("status.json")?;
        client.family = FirmwareFamily::detect(&info.fw_ver);

        Ok(client)
    }

    // -- Read-only API calls ------------------------------------------------

    /// System Information — `GET /status.json`
    pub fn get_system_info(&self) -> Result<SystemInfo> {
        self.get_json("status.json")
    }

    /// Network Settings — `GET /network_settings.json`
    pub fn get_network_settings(&self) -> Result<NetworkSettings> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("network_settings.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("network_settings.json")?;
                v1::parse_network_settings(&raw)
            }
        }
    }

    /// Port Settings — `GET /port_setting_load.json`
    pub fn get_port_settings(&self) -> Result<PortSettingsResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("port_setting_load.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("port_setting_load.json")?;
                v1::parse_port_settings(&raw)
            }
        }
    }

    /// Port Statistics — `GET /port_statistics.json`
    pub fn get_port_statistics(&self) -> Result<PortStatisticsResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("port_statistics.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("port_statistics.json")?;
                v1::parse_port_statistics(&raw)
            }
        }
    }

    /// Dynamic MAC Table — `GET /mac_get_dynamic_mac_entries.json`
    ///
    /// V2's API returns `data: [{...}, {...}]` lines (JSON with `data:`
    /// prefix); V1 returns a `{total_entries, Idx_0..Idx_N}` object.
    pub fn get_dynamic_mac_entries(&self) -> Result<Vec<MacEntry>> {
        match self.family {
            FirmwareFamily::V2 => {
                let raw = self.get_raw("mac_get_dynamic_mac_entries.json")?;
                parse_mac_entries(&raw)
            }
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("mac_get_dynamic_mac_entries.json")?;
                v1::parse_dynamic_mac_entries(&raw)
            }
        }
    }

    /// Static MAC Table — `GET /mac_get_static_mac_entries.json`
    pub fn get_static_mac_entries(&self) -> Result<Vec<StaticMacEntry>> {
        match self.family {
            FirmwareFamily::V2 => {
                let raw = self.get_raw("mac_get_static_mac_entries.json")?;
                parse_static_mac_entries(&raw)
            }
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("mac_get_static_mac_entries.json")?;
                v1::parse_static_mac_entries(&raw)
            }
        }
    }

    /// Loop Detection Status — `GET /port_loop_status.json`
    pub fn get_loop_status(&self) -> Result<LoopStatusResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("port_loop_status.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("port_loop_status.json")?;
                v1::parse_loop_status(&raw)
            }
        }
    }

    /// STP Config — `GET /stp.json`
    pub fn get_stp_config(&self) -> Result<StpConfig> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("stp.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("stp.json")?;
                v1::parse_stp_config(&raw)
            }
        }
    }

    /// Port VLAN Config — `GET /port_vlan.json` (V2) or `GET /port_vlan_cfg.json` (V1)
    pub fn get_port_vlan(&self) -> Result<PortVlanResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("port_vlan.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("port_vlan_cfg.json")?;
                v1::parse_port_vlan(&raw)
            }
        }
    }

    /// All Port PVIDs — `GET /all_port_pvid.json` (V2 only; no V1 equivalent found)
    pub fn get_all_port_pvids(&self) -> Result<PortPvidsResponse> {
        self.get_json("all_port_pvid.json")
    }

    /// IGMP Config — `GET /igmp_config.json`
    pub fn get_igmp_config(&self) -> Result<IgmpConfig> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("igmp_config.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("igmp_config.json")?;
                v1::parse_igmp_config(&raw)
            }
        }
    }

    /// Storm Control Config — `GET /storm_ctrl_cfg.json`
    ///
    /// On V1 firmware this is switch-global (one rate for the whole switch,
    /// not per-port/per-traffic-type); see `v1::parse_storm_control`.
    pub fn get_storm_control(&self) -> Result<StormControlResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("storm_ctrl_cfg.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("storm_ctrl_cfg.json")?;
                v1::parse_storm_control(&raw)
            }
        }
    }

    /// Port Mirror Config — `GET /port_mirror.json`
    pub fn get_port_mirror(&self) -> Result<PortMirrorResponse> {
        match self.family {
            FirmwareFamily::V2 => self.get_json("port_mirror.json"),
            FirmwareFamily::V1 => {
                let raw = self.get_json_value("port_mirror.json")?;
                v1::parse_port_mirror(&raw)
            }
        }
    }

    /// Link Aggregation Config — `GET /port_trunk_cfg.json`
    ///
    /// V1's response shape matches `TrunkConfigResponse` field-for-field
    /// already (flattened raw catches the rest), so no V1-specific parsing
    /// is needed for reads — only writes differ.
    pub fn get_trunk_config(&self) -> Result<TrunkConfigResponse> {
        self.get_json("port_trunk_cfg.json")
    }

    // -- Write API calls ----------------------------------------------------

    /// Set device description — `POST /set_des.json`
    /// **Confidence: certain** (captured from web UI, live-tested on V1)
    pub fn set_description(&self, description: &str) -> Result<WriteResponse> {
        let body = match self.family {
            FirmwareFamily::V2 => serde_json::json!({"des": description}),
            FirmwareFamily::V1 => v1::set_description_body(description),
        };
        self.post_json("set_des.json", &body)
    }

    /// Apply port settings — `POST /apply_user_port_setting.json`
    /// **Confidence: likely on V2** (inferred from GET response shape);
    /// **certain on V1** (captured from web UI JS).
    pub fn set_port_settings(&self, request: &PortSettingsApplyRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("apply_user_port_setting.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_port_settings()?;
                let body = v1::build_port_settings_apply_body(&current, request)?;
                self.post_json("apply_user_port_setting.json", &body)
            }
        }
    }

    /// Update IPv4 network settings — `POST /network_settings_ipv4.json`
    /// **Confidence: likely on V2** (inferred from GET response);
    /// **certain on V1** (captured from web UI JS). V1 does not expose DNS
    /// server configuration through this endpoint at all.
    pub fn set_network_settings(&self, request: &NetworkSettingsRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("network_settings_ipv4.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_network_settings()?;
                let body = v1::build_network_settings_body(&current, request)?;
                self.post_json_custom_success(
                    "network_settings_ipv4.json",
                    &body,
                    v1::network_settings_response_is_success,
                )
            }
        }
    }

    /// Set per-port VLAN config — `POST /port_vlan.json` (V2) or
    /// `POST /port_vlan_cfg.json` (V1).
    /// **Confidence: speculative on both** — best-effort mapping of V1's
    /// bridge-port enable/tag/untag flags onto V2's PVID+FrameType model.
    /// V1's write replaces the whole 10-port table in one POST; untouched
    /// ports are carried forward from the switch's *raw* current response
    /// (not decoded through the canonical model — see `v1::build_port_vlan_body`
    /// for why: an earlier version reconstructed them lossily and took the
    /// test switch off the network entirely).
    pub fn set_port_vlan(&self, request: &PortVlanRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("port_vlan.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_json_value("port_vlan_cfg.json")?;
                let body = v1::build_port_vlan_body(&current, request)?;
                let resp = self.post_json("port_vlan_cfg.json", &body)?;
                if resp.is_success() {
                    self.post_json("save_port_vlan_map.json", &serde_json::json!({}))?;
                }
                Ok(resp)
            }
        }
    }

    /// Update IGMP snooping config — `POST /igmp_config.json`
    /// **Confidence: speculative on V2**; **likely on V1** (captured from
    /// web UI JS, but the checkbox-omission convention is unconfirmed on
    /// real hardware).
    pub fn set_igmp_config(&self, request: &IgmpConfigRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("igmp_config.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_igmp_config()?;
                let body = v1::build_igmp_config_body(&current, request);
                self.post_json("igmp_config.json", &body)
            }
        }
    }

    /// Set storm control rates — `POST /storm_ctrl_cfg.json`
    /// **Confidence: speculative on V2.** Not supported on V1: that
    /// firmware's storm control is a single switch-wide rate, not
    /// per-port/per-traffic-type, so this tool's per-port request shape has
    /// no faithful translation.
    pub fn set_storm_control(&self, request: &StormControlRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("storm_ctrl_cfg.json", &body)
            }
            FirmwareFamily::V1 => bail!(
                "This switch's firmware (V1/GoodTop) only supports a single \
                 switch-wide storm control rate, not independent per-port \
                 broadcast/multicast/unicast rates. Not supported by this tool."
            ),
        }
    }

    /// Configure port mirroring — `POST /port_mirror.json`
    /// **Confidence: speculative on V2**; **likely on V1** (captured from
    /// web UI JS). V1 only supports the same port set for ingress and
    /// egress — see `v1::build_port_mirror_body`.
    pub fn set_port_mirror(&self, request: &PortMirrorRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("port_mirror.json", &body)
            }
            FirmwareFamily::V1 => {
                let body = v1::build_port_mirror_body(request)?;
                self.post_json("port_mirror.json", &body)
            }
        }
    }

    /// Update STP configuration — `POST /stp.json`
    /// **Confidence: speculative on V2**; **likely on V1** for the
    /// enable/disable toggle this tool actually exposes (captured from web
    /// UI JS). Per-port edge-port flags are never touched by this tool on
    /// either firmware family.
    pub fn set_stp_config(&self, request: &StpConfigRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("stp.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_stp_config()?;
                let body = v1::build_stp_config_body(&current.stp_rstp_mode, request);
                self.post_json("stp.json", &body)
            }
        }
    }

    /// Set trunk/LACP config — `POST /port_trunk_cfg.json`
    /// **Confidence: speculative on both** — the firmware's own JS is
    /// internally inconsistent about port-index mapping for this endpoint's
    /// Link Up/Down display; the write path (what this sends) is
    /// consistently mapped. See `v1::build_trunk_config_body`.
    pub fn set_trunk_config(&self, request: &TrunkConfigRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("port_trunk_cfg.json", &body)
            }
            FirmwareFamily::V1 => {
                let current = self.get_json_value("port_trunk_cfg.json")?;
                let body = v1::build_trunk_config_body(&current, request)?;
                self.post_json("port_trunk_cfg.json", &body)
            }
        }
    }

    /// Configure loop protection — `POST /port_lock_cfg.json`
    /// Not supported on V1: that firmware's `port_lock_cfg.json` is
    /// per-port security locking, not the detection-interval/recovery-time
    /// loop protection this tool's request models — despite the confusingly
    /// similar name and the firmware UI's own "loop detection" page title.
    pub fn set_loop_protection(&self, request: &LoopProtectionRequest) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::to_value(request)?;
                self.post_json("port_lock_cfg.json", &body)
            }
            FirmwareFamily::V1 => bail!(
                "This switch's firmware (V1/GoodTop) does not expose loop \
                 detection interval/recovery-time configuration — its \
                 port_lock_cfg endpoint is per-port security locking, a \
                 different feature. Not supported by this tool."
            ),
        }
    }

    /// Save running config to startup.
    /// V2: `POST /save_all_configs.json`. V1 has no single "save everything"
    /// endpoint — only port settings and VLAN config have a separate
    /// apply-then-save step (other V1 settings persist immediately on
    /// their `set_*` POST), so this saves both of those.
    pub fn save_config(&self) -> Result<WriteResponse> {
        match self.family {
            FirmwareFamily::V2 => {
                let body = serde_json::json!({});
                self.post_json("save_all_configs.json", &body)
            }
            FirmwareFamily::V1 => {
                self.post_json("save_user_port_setting.json", &serde_json::json!({}))?;
                self.post_json("save_port_vlan_map.json", &serde_json::json!({}))
            }
        }
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
    ///
    /// Not supported on V1: that firmware's native config backup
    /// (`/config/download`) is a custom binary key=value blob, not JSON, and
    /// wasn't reverse-engineered here — use the switch's own web UI for
    /// backup/restore on V1 firmware for now.
    pub fn backup_config(&self) -> Result<String> {
        if self.family == FirmwareFamily::V1 {
            bail!(
                "Backup/restore isn't supported on this switch's firmware \
                 (V1/GoodTop) yet — its native config format is a custom \
                 binary blob, not JSON. Use the switch's own web UI \
                 (System > Backup/Restore) instead."
            );
        }
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
        if self.family == FirmwareFamily::V1 {
            bail!(
                "Backup/restore isn't supported on this switch's firmware \
                 (V1/GoodTop) yet — its native config format is a custom \
                 binary blob, not JSON. Use the switch's own web UI \
                 (System > Backup/Restore) instead."
            );
        }
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

    /// Fetch an endpoint and deserialize as a generic JSON value (used for
    /// V1 endpoints whose shape requires manual, port-remapped extraction
    /// rather than a direct `#[derive(Deserialize)]`).
    fn get_json_value(&self, endpoint: &str) -> Result<serde_json::Value> {
        self.get_json(endpoint)
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

    /// Like `post_json`, but for endpoints with a non-standard success
    /// convention (e.g. V1's `network_settings_ipv4.json`, which returns
    /// `{"alert_msg":"Success"}` instead of an empty body).
    fn post_json_custom_success<T: serde::Serialize>(
        &self,
        endpoint: &str,
        body: &T,
        is_success: impl Fn(&str) -> bool,
    ) -> Result<WriteResponse> {
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

        if response_body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        Ok(WriteResponse {
            result: Some(
                if is_success(&response_body) {
                    "success"
                } else {
                    "failure"
                }
                .to_string(),
            ),
        })
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
