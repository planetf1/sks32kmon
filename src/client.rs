use anyhow::{bail, Context, Result};
use md5::{Digest, Md5};
use reqwest::blocking::Client as HttpClient;
use reqwest::cookie::Jar;
use std::sync::Arc;
use std::time::Duration;

use crate::models::*;

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

    // -- Internal helpers ---------------------------------------------------

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

        let body = resp.text().with_context(|| format!("Read {} failed", endpoint))?;

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

        let body = resp.text().with_context(|| format!("Read {} failed", endpoint))?;

        if body.contains("login.html") {
            bail!("Session expired — please reconnect");
        }

        Ok(body)
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
