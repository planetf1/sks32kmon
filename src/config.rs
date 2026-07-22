use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

/// A single switch definition from the config file.
#[derive(Debug, Clone, Deserialize)]
pub struct SwitchDef {
    /// Optional friendly name (used for display and CLI targeting).
    pub name: Option<String>,
    /// Hostname or IP address.
    pub host: String,
    /// Login username (defaults to "admin" if not set).
    #[serde(default = "default_user")]
    pub user: String,
    /// Login password.
    pub password: String,
}

fn default_user() -> String {
    "admin".to_string()
}

/// The full configuration file.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    /// List of managed switches.
    #[serde(default)]
    pub switch: Vec<SwitchDef>,
}

/// Resolved switch target — either from config or ad-hoc CLI args.
#[derive(Debug, Clone)]
pub struct SwitchTarget {
    pub name: String,
    pub host: String,
    pub user: String,
    pub password: String,
}

// ---------------------------------------------------------------------------
// Default config path discovery
// ---------------------------------------------------------------------------

/// Returns the default config file path: `~/.config/sks3200/config.toml`
pub fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".config")
        .join("sks3200")
        .join("config.toml")
}

/// Load configuration from a TOML file at `path`.
pub fn load_config_file(path: &std::path::Path) -> Result<ConfigFile> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;
    let config: ConfigFile = toml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;
    Ok(config)
}

/// Build config from the default path if it exists.
pub fn load_default_config() -> Option<ConfigFile> {
    let path = default_config_path();
    if path.exists() {
        load_config_file(&path).ok()
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Switch resolution
// ---------------------------------------------------------------------------

/// Given CLI `--switch` args (can be names from config, IPs, or both), plus
/// optional ad-hoc `--user` and `--password`, produce the resolved list of
/// targets to operate on.
///
/// Resolution rules:
/// 1. If `switches_arg` is non-empty:
///    - Each entry looked up by name in config (case-sensitive).
///    - If no config match, treat as a raw IP/host and pair with ad-hoc creds.
///    - Ad-hoc creds: `cli_user`/`cli_password` if given, else error.
/// 2. If `switches_arg` is empty and config exists → use all config switches.
/// 3. If `switches_arg` is empty and no config → fall back to `SKS3200_HOST`
///    env var or error.
pub fn resolve_switches(
    switches_arg: &[String],
    cli_user: Option<&str>,
    cli_password: Option<&str>,
    config: &Option<ConfigFile>,
) -> Result<Vec<SwitchTarget>> {
    if !switches_arg.is_empty() {
        // User explicitly listed targets
        let config_switches: &[SwitchDef] = match config {
            Some(cfg) => &cfg.switch,
            None => &[],
        };
        let mut targets = Vec::new();

        for item in switches_arg {
            // Try to look up by name in config
            let found = config_switches.iter().find(|s| {
                s.name.as_deref() == Some(item.as_str())
            });

            match found {
                Some(def) => targets.push(SwitchTarget {
                    name: def.name.clone().unwrap_or_else(|| def.host.clone()),
                    host: def.host.clone(),
                    user: def.user.clone(),
                    password: def.password.clone(),
                }),
                None => {
                    // Treat as raw IP/host — need ad-hoc creds
                    let user = cli_user.unwrap_or("admin").to_string();
                    let password = cli_password.map(|s| s.to_string()).unwrap_or_else(|| {
                        // Try env var as last resort
                        std::env::var("SKS3200_PASSWORD")
                            .unwrap_or_default()
                    });
                    if password.is_empty() {
                        bail!(
                            "No password for switch '{}'. Use --password, SKS3200_PASSWORD, \
                             or add it to the config file at {}",
                            item,
                            default_config_path().display()
                        );
                    }
                    targets.push(SwitchTarget {
                        name: item.clone(),
                        host: item.clone(),
                        user,
                        password,
                    });
                }
            }
        }

        Ok(targets)
    } else if let Some(cfg) = config {
        if cfg.switch.is_empty() {
            bail!(
                "No switches configured. Add switches to {} or use --switch.",
                default_config_path().display()
            );
        }
        // Use all configured switches
        Ok(cfg
            .switch
            .iter()
            .map(|s| SwitchTarget {
                name: s.name.clone().unwrap_or_else(|| s.host.clone()),
                host: s.host.clone(),
                user: s.user.clone(),
                password: s.password.clone(),
            })
            .collect())
    } else {
        // No config, no --switch — try env var
        let host = std::env::var("SKS3200_HOST")
            .unwrap_or_else(|_| "192.168.100.7".to_string());
        let user = cli_user.unwrap_or("admin").to_string();
        let password = cli_password.map(|s| s.to_string()).unwrap_or_else(|| {
            std::env::var("SKS3200_PASSWORD").unwrap_or_default()
        });
        if password.is_empty() {
            bail!(
                "No password. Use --password, SKS3200_PASSWORD, or configure switches in {}",
                default_config_path().display()
            );
        }
        Ok(vec![SwitchTarget {
            name: host.clone(),
            host,
            user,
            password,
        }])
    }
}

/// Generate a TOML config file template.
pub fn generate_template() -> &'static str {
    r#"# SKS3200 Switch Manager Configuration
#
# Add your switches here. The tool will query all configured switches
# when no --switch flag is given.
#
#   name     – Friendly label (optional, used for CLI targeting)
#   host     – IP address or hostname (required)
#   user     – Login username (defaults to "admin")
#   password – Login password (required)

[[switch]]
name = "main"
host = "192.168.100.7"
user = "admin"
password = "changeme"

[[switch]]
name = "secondary"
host = "192.168.100.8"
password = "changeme"
"#
}
