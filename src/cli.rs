use std::path::Path;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use colored::*;

use crate::api::ApiClient;
use crate::config::{self, SwitchTarget};
use crate::models::PortMirrorEntry;
#[cfg(feature = "tui")]
use crate::tui;
use crate::write_models::*;

// ---------------------------------------------------------------------------
// CLI argument definition
// ---------------------------------------------------------------------------

#[derive(Parser, Debug)]
#[command(
    name = "sks3200",
    about = "Manage XikeStor SKS3200-8E2X switches",
    version,
    long_about = concat!(
        "Query one or more SKS3200-8E2X switches.\n\n",
        "Switches can be pre-configured in ~/.config/sks3200/config.toml.\n",
        "When no --switch is given, ALL configured switches are queried.\n",
        "Use -s <name> to target specific switches by config name or IP."
    )
)]
pub struct Args {
    /// Switch names or IPs (from config, or ad-hoc). Repeatable, comma-separated.
    /// Defaults to all configured switches if omitted.
    #[arg(
        short = 's',
        long = "switch",
        env = "SKS3200_HOST",
        value_delimiter = ','
    )]
    pub switches: Vec<String>,

    /// Path to config file (default: ~/.config/sks3200/config.toml)
    #[arg(short = 'c', long = "config")]
    pub config: Option<String>,

    /// Login username (fallback for ad-hoc switches not in config)
    #[arg(short = 'u', long = "user")]
    pub user: Option<String>,

    /// Login password (fallback for ad-hoc switches not in config)
    #[arg(short = 'p', long = "password", env = "SKS3200_PASSWORD")]
    pub password: Option<String>,

    /// Output raw JSON
    #[arg(short = 'j', long = "json")]
    pub json: bool,

    /// Suppress informational messages about mock mode
    #[arg(long = "quiet", short = 'q')]
    pub quiet: bool,

    /// Actually apply write operations to the switch (affects real switch!)
    #[arg(long = "apply")]
    pub apply: bool,

    /// Skip confirmation prompts for destructive operations
    #[arg(long = "yes", short = 'y')]
    pub yes: bool,

    /// Force mock mode for all commands (useful for testing)
    #[arg(long = "mock", hide = true)]
    pub mock: bool,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Show switch state (read-only queries)
    #[command(subcommand)]
    Show(ShowCommand),

    /// Configure switch settings
    #[command(subcommand)]
    Set(SetCommand),

    /// Clear runtime state
    #[command(subcommand)]
    Clear(ClearCommand),

    /// Save running config to startup
    Save,

    /// Backup configuration to a file (or stdout)
    Backup {
        /// Output file path (defaults to stdout)
        #[arg(short = 'o', long = "output")]
        output: Option<String>,
    },

    /// Restore configuration from a backup file
    Restore {
        /// Path to backup JSON file
        file: String,
    },

    /// Reboot the switch (requires --yes --apply)
    Reboot,

    /// Factory reset the switch (requires --yes --apply, DESTRUCTIVE!)
    FactoryReset,

    /// Launch TUI dashboard (live monitoring)
    #[cfg(feature = "tui")]
    Monitor {
        /// Use mock mode instead of connecting to real switch
        #[arg(long)]
        mock: bool,
    },

    /// Generate a sample config file template (prints to stdout)
    ConfigInit,
}

// ---------------------------------------------------------------------------
// Nested subcommand enums
// ---------------------------------------------------------------------------

/// Show switch state
#[derive(Subcommand, Debug)]
pub enum ShowCommand {
    /// System information (temperature, IP, MAC, firmware)
    Status,
    /// Port status and settings
    Ports,
    /// Port traffic statistics
    Statistics {
        /// Continuously refresh (every 2s)
        #[arg(short = 'w', long = "watch")]
        watch: bool,
    },
    /// Dynamic MAC address table
    Mac,
    /// Static MAC address table
    StaticMac,
    /// Link aggregation / trunk status
    Trunk,
    /// VLAN configuration
    Vlan,
    /// Spanning Tree Protocol status
    Stp,
    /// Loop protection status
    Loop,
    /// IGMP snooping configuration
    Igmp,
    /// Storm control configuration
    Storm,
    /// Port mirror configuration
    Mirror,
    /// Network settings (IP, gateway, DNS)
    Network,
    /// Show all information at once
    All,
}

/// Configure switch settings
#[derive(Subcommand, Debug)]
pub enum SetCommand {
    /// Configure port (status, speed, flow control)
    Port {
        /// Port number (1-10)
        port: u32,
        /// Port admin status: enabled or disabled
        #[arg(long, value_parser = ["enabled", "disabled"])]
        status: Option<String>,
        /// Speed/duplex: auto, 100, 1000, 2500
        #[arg(long, value_parser = ["auto", "100", "1000", "2500"])]
        speed: Option<String>,
        /// Flow control: on or off
        #[arg(long, value_parser = ["on", "off"])]
        flow: Option<String>,
    },
    /// Set device description
    Description {
        /// New description text
        text: String,
    },
    /// Configure VLAN per-port (PVID, frame type)
    Vlan {
        /// Port number (1-10)
        port: u32,
        /// Port VLAN ID (1-4094)
        #[arg(long)]
        pvid: Option<u32>,
        /// Frame type: all, tagged, untagged
        #[arg(long, value_parser = ["all", "tagged", "untagged"])]
        frame_type: Option<String>,
    },
    /// Configure IGMP snooping
    Igmp {
        /// IGMP snooping: on or off
        #[arg(long, value_parser = ["on", "off"])]
        snooping: Option<String>,
        /// Fast leave: on or off
        #[arg(long, value_parser = ["on", "off"])]
        fast_leave: Option<String>,
        /// Report flood: on or off
        #[arg(long, value_parser = ["on", "off"])]
        report_flood: Option<String>,
    },
    /// Update IPv4 network settings
    Network {
        /// IP address
        #[arg(long)]
        ip: Option<String>,
        /// Netmask
        #[arg(long)]
        mask: Option<String>,
        /// Gateway IP
        #[arg(long)]
        gateway: Option<String>,
        /// DNS server
        #[arg(long)]
        dns: Option<String>,
        /// Enable DHCP: on or off
        #[arg(long, value_parser = ["on", "off"])]
        dhcp: Option<String>,
    },
    /// Configure storm control per port
    Storm {
        /// Port number (1-10)
        port: u32,
        /// Broadcast rate limit (Kbps, 0 to disable)
        #[arg(long, default_value_t = 0)]
        broadcast: u32,
        /// Multicast rate limit (Kbps)
        #[arg(long, default_value_t = 0)]
        multicast: u32,
        /// Unicast rate limit (Kbps)
        #[arg(long, default_value_t = 0)]
        unicast: u32,
        /// Unknown multicast rate limit (Kbps)
        #[arg(long, default_value_t = 0)]
        unmcast: u32,
    },
    /// Enable or disable Spanning Tree Protocol
    Stp {
        /// Action: enable or disable
        #[arg(long, value_parser = ["enable", "disable"])]
        action: String,
    },
    /// Configure port mirroring
    Mirror {
        /// Monitor port ID (0 to disable mirroring, 1-10 to set destination)
        #[arg(long)]
        monitor_port: Option<u32>,
        /// Ports with ingress mirroring enabled (comma-separated, e.g. "1,3,5")
        #[arg(long)]
        source_ingress: Option<String>,
        /// Ports with egress mirroring enabled (comma-separated)
        #[arg(long)]
        source_egress: Option<String>,
    },
    /// Configure trunk/LACP per-port
    Trunk {
        /// Port number (1-10)
        port: u32,
        /// Trunk type: static, lacp, or none
        #[arg(long = "trunk-type", value_parser = ["static", "lacp", "none"])]
        trunk_type: Option<String>,
        /// Aggregation group index (0-15)
        #[arg(long)]
        group: Option<u32>,
    },
    /// Configure loop protection
    LoopProtection {
        /// Enable loop detection (true/false)
        #[arg(long)]
        enable: Option<String>,
        /// Detection interval in seconds
        #[arg(long)]
        interval: Option<u32>,
        /// Recovery time in seconds
        #[arg(long)]
        recover: Option<u32>,
    },
}

/// Clear runtime state
#[derive(Subcommand, Debug)]
pub enum ClearCommand {
    /// Clear port statistics counters
    Statistics,
    /// Clear dynamic MAC address table
    Mac,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run(args: Args) -> Result<()> {
    // Config init doesn't need switch connections
    if let Command::ConfigInit = &args.command {
        print!("{}", config::generate_template());
        return Ok(());
    }

    // 1. Load config
    let config = match &args.config {
        Some(path) => {
            let p = Path::new(path);
            if p.exists() {
                Some(config::load_config_file(p)?)
            } else {
                eprintln!("Config file not found: {}", path);
                None
            }
        }
        None => config::load_default_config(),
    };

    // 2. Resolve switch targets
    let targets = config::resolve_switches(
        &args.switches,
        args.user.as_deref(),
        args.password.as_deref(),
        &config,
    )?;

    // 3. Dispatch
    match &args.command {
        #[cfg(feature = "tui")]
        Command::Monitor { mock } => {
            tui::run_tui(&targets, *mock)?;
        }
        cmd => {
            for (i, target) in targets.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                // Determine client type based on command and flags
                let use_mock = args.mock || (is_write_command(cmd) && !args.apply);

                let client = if use_mock {
                    if is_write_command(cmd) && !args.quiet && !args.apply {
                        eprintln!(
                            "{} Running in mock mode (no changes applied). Use --apply to write to the switch.",
                            "ℹ".yellow()
                        );
                    }
                    ApiClient::connect_mock(&target.host, &target.user, &target.password)?
                } else {
                    ApiClient::connect_real(&target.host, &target.user, &target.password)?
                };
                run_command_on(cmd, &client, args.json, args.yes, target)?;
            }
        }
    }

    Ok(())
}

/// Determine if a command modifies switch state.
fn is_write_command(cmd: &Command) -> bool {
    matches!(
        cmd,
        Command::Set { .. }
            | Command::Clear { .. }
            | Command::Save
            | Command::Restore { .. }
            | Command::Reboot
            | Command::FactoryReset
    )
}

/// Run a single command against one connected switch.
fn run_command_on(
    cmd: &Command,
    client: &ApiClient,
    json: bool,
    yes: bool,
    target: &SwitchTarget,
) -> Result<()> {
    if !json {
        // Print switch header when it has a friendly name or in multi-switch mode
        let label = if target.name != target.host {
            format!("{} ({})", target.name.cyan().bold(), target.host)
        } else {
            target.host.clone()
        };
        println!("═══ {} ═══", label);
    }

    match cmd {
        Command::Show(sc) => match sc {
            ShowCommand::Status => cmd_status(client, json),
            ShowCommand::Ports => cmd_ports(client, json),
            ShowCommand::Statistics { watch } => cmd_statistics(client, json, *watch),
            ShowCommand::Mac => cmd_mac(client, json),
            ShowCommand::StaticMac => cmd_static_mac(client, json),
            ShowCommand::Trunk => cmd_trunk(client, json),
            ShowCommand::Vlan => cmd_vlan(client, json),
            ShowCommand::Stp => cmd_stp(client, json),
            ShowCommand::Loop => cmd_loop(client, json),
            ShowCommand::Igmp => cmd_igmp(client, json),
            ShowCommand::Storm => cmd_storm(client, json),
            ShowCommand::Mirror => cmd_mirror(client, json),
            ShowCommand::Network => cmd_network(client, json),
            ShowCommand::All => cmd_all(client, json),
        },
        Command::Set(sc) => match sc {
            SetCommand::Port {
                port,
                status,
                speed,
                flow,
            } => cmd_port_set(
                client,
                json,
                *port,
                status.as_deref(),
                speed.as_deref(),
                flow.as_deref(),
            ),
            SetCommand::Description { text } => cmd_description_set(client, json, text),
            SetCommand::Vlan {
                port,
                pvid,
                frame_type,
            } => cmd_vlan_set(client, json, *port, *pvid, frame_type.as_deref()),
            SetCommand::Igmp {
                snooping,
                fast_leave,
                report_flood,
            } => cmd_igmp_set(
                client,
                json,
                snooping.as_deref(),
                fast_leave.as_deref(),
                report_flood.as_deref(),
            ),
            SetCommand::Network {
                ip,
                mask,
                gateway,
                dns,
                dhcp,
            } => cmd_network_set(
                client,
                json,
                ip.as_deref(),
                mask.as_deref(),
                gateway.as_deref(),
                dns.as_deref(),
                dhcp.as_deref(),
            ),
            SetCommand::Storm {
                port,
                broadcast,
                multicast,
                unicast,
                unmcast,
            } => cmd_storm_set(
                client, json, *port, *broadcast, *multicast, *unicast, *unmcast,
            ),
            SetCommand::Stp { action } => cmd_stp_set(client, json, action),
            SetCommand::Mirror {
                monitor_port,
                source_ingress,
                source_egress,
            } => cmd_mirror_set(
                client,
                json,
                monitor_port,
                source_ingress.as_deref(),
                source_egress.as_deref(),
            ),
            SetCommand::Trunk {
                port,
                trunk_type,
                group,
            } => cmd_trunk_set(client, json, *port, trunk_type.as_deref(), *group),
            SetCommand::LoopProtection {
                enable,
                interval,
                recover,
            } => cmd_loop_protection_set(client, json, enable.as_deref(), *interval, *recover),
        },
        Command::Clear(cc) => match cc {
            ClearCommand::Statistics => cmd_statistics_clear(client, json),
            ClearCommand::Mac => cmd_mac_clear(client, json),
        },
        Command::Save => cmd_config_save(client, json),
        Command::Backup { output } => cmd_backup(client, json, output.as_deref()),
        Command::Restore { file } => cmd_restore(client, json, file, yes),
        Command::Reboot => cmd_reboot(client, json, yes),
        Command::FactoryReset => cmd_factory_reset(client, json, yes),

        Command::ConfigInit => unreachable!(),
        #[cfg(feature = "tui")]
        Command::Monitor { .. } => unreachable!(),
    }
}

// ===========================================================================
// Command implementation functions (each takes one ApiClient)
// ===========================================================================

fn cmd_status(client: &ApiClient, json: bool) -> Result<()> {
    let info = client.get_system_info()?;
    let net = client.get_network_settings().ok();

    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }

    println!("{}", "╌".repeat(50));
    println!(" {}  {}", "SKS3200-8E2X".bold().white(), info.des.cyan());
    println!("{}", "╌".repeat(50));
    println!("  {:<18}  {}", "Firmware:", info.fw_ver);
    println!("  {:<18}  {}", "Hardware:", info.hw_ver);
    println!("  {:<18}  {}", "MAC Address:", info.sys_macaddr);
    println!("  {:<18}  {}", "IP Address:", info.sys_ipv4);
    println!("  {:<18}  {}°C", "Temperature:", info.temperature.yellow());

    if let Some(net) = net {
        println!("  {:<18}  {}", "Netmask:", net.netmask);
        println!("  {:<18}  {}", "Gateway:", net.gateway);
        println!("  {:<18}  {}", "DNS:", net.dns_server);
        println!(
            "  {:<18}  {}",
            "DHCP:",
            if net.dhcp_enabled == "1" {
                "Enabled"
            } else {
                "Static"
            }
        );
    }

    Ok(())
}

fn cmd_ports(client: &ApiClient, json: bool) -> Result<()> {
    let ports = client.get_port_settings()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&ports)?);
        return Ok(());
    }

    println!(
        " {}  Mode: {}  Active: {}/{}",
        "PORT SETTINGS".bold().white(),
        ports.port_mode,
        ports.active_port_count().to_string().green(),
        ports.port_num
    );
    println!("{}", "╌".repeat(90));
    println!(
        " {} {} {} {} {} {}",
        pad_str("Port", 7),
        pad_str("Status", 10),
        pad_str("Actual Speed", 20),
        pad_str("Config", 20),
        pad_str("Flow Ctrl", 12),
        pad_str("EEE", 10),
    );
    println!("{}", "─".repeat(90));

    for p in ports.ports() {
        let port_str = pad_str(format!("Port {}", p.port_id), 7).bold().to_string();
        let status_str: String = if p.port_status == "Enabled" {
            pad_str("Enabled", 10).green().to_string()
        } else {
            pad_str("Disabled", 10).red().to_string()
        };
        let speed_str: String = if p.spd_duplex_actual == "Link Down" {
            pad_str(&p.spd_duplex_actual, 20).red().to_string()
        } else {
            pad_str(&p.spd_duplex_actual, 20).green().to_string()
        };
        let config_str = pad_str(&p.spd_duplex_cfg, 20);
        let flow_str: String = if p.flow_ctrl_actual == "On" {
            pad_str("On", 12).green().to_string()
        } else {
            pad_str("Off", 12).yellow().to_string()
        };
        let eee_str: String = match p.eee_status.as_str() {
            "eee_active" => pad_str("Active", 10).green().to_string(),
            "eee_inactive" => pad_str("Inactive", 10).yellow().to_string(),
            _ => pad_str("N/A", 10).dimmed().to_string(),
        };

        println!(
            " {} {} {} {} {} {}",
            port_str, status_str, speed_str, config_str, flow_str, eee_str,
        );
    }

    Ok(())
}

fn cmd_statistics(client: &ApiClient, json: bool, watch: bool) -> Result<()> {
    loop {
        let stats = client.get_port_statistics()?;

        if json {
            println!("{}", serde_json::to_string_pretty(&stats)?);
            if !watch {
                break;
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
            continue;
        }

        println!(" {}", "PORT STATISTICS".bold().white());
        println!("{}", "╌".repeat(100));
        println!(
            " {} {} {} {} {} {}",
            pad_str("Port", 7),
            pad_str("Status", 16),
            pad_str("Tx Good", 16),
            pad_str("Tx Bad", 16),
            pad_str("Rx Good", 16),
            pad_str("Rx Bad", 16),
        );
        println!("{}", "─".repeat(100));

        for p in stats.ports() {
            let port_str = pad_str(format!("Port {}", p.port_id), 7).bold().to_string();
            let status_str: String = if p.link_status == "Link Down" {
                pad_str(&p.link_status, 16).red().to_string()
            } else {
                pad_str(&p.link_status, 16).green().to_string()
            };
            let tx_good = pad_str(format_num(&p.tx_good_pkt), 16);
            let tx_bad = pad_str(format_num(&p.tx_bad_pkt), 16).red().to_string();
            let rx_good = pad_str(format_num(&p.rx_good_pkt), 16);
            let rx_bad = pad_str(format_num(&p.rx_bad_pkt), 16).red().to_string();

            println!(
                " {} {} {} {} {} {}",
                port_str, status_str, tx_good, tx_bad, rx_good, rx_bad,
            );
        }

        if !watch {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }

    Ok(())
}

fn cmd_mac(client: &ApiClient, json: bool) -> Result<()> {
    let entries = client.get_dynamic_mac_entries()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!(
        " {}  {} entries",
        "DYNAMIC MAC TABLE".bold().white(),
        entries.len().to_string().cyan()
    );
    println!("{}", "╌".repeat(90));
    println!(
        " {:<3} {:<22} {:<6} {:<6} {:<6}",
        "#", "MAC Address", "VLAN", "Port", "Age"
    );
    println!("{}", "─".repeat(90));

    for (i, e) in entries.iter().enumerate() {
        println!(
            " {:<3} {:<22} {:<6} {:<6} {:<6}s",
            i + 1,
            e.mac_addr,
            e.vlan_id,
            e.port_id,
            e.age_timer,
        );
    }

    Ok(())
}

fn cmd_static_mac(client: &ApiClient, json: bool) -> Result<()> {
    let entries = client.get_static_mac_entries()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    println!(
        " {}  {} entries",
        "STATIC MAC TABLE".bold().white(),
        entries.len().to_string().cyan()
    );
    println!("{}", "╌".repeat(60));
    println!(
        " {:<3} {:<22} {:<6} {:<6}",
        "#", "MAC Address", "VLAN", "Port"
    );
    println!("{}", "─".repeat(60));

    for (i, e) in entries.iter().enumerate() {
        println!(
            " {:<3} {:<22} {:<6} {:<6}",
            i + 1,
            e.mac_addr,
            e.vlan_id,
            e.port_id
        );
    }

    Ok(())
}

fn cmd_trunk(client: &ApiClient, json: bool) -> Result<()> {
    let trunk = client.get_trunk_config()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&trunk)?);
        return Ok(());
    }

    println!(
        " {}  System Priority: {}",
        "LINK AGGREGATION".bold().white(),
        trunk.system_priority
    );
    println!("{}", "╌".repeat(60));

    let raw = &trunk.raw;
    println!(
        " {} {} {}",
        pad_str("Port", 7),
        pad_str("Type", 12),
        pad_str("Group", 12),
    );
    println!("{}", "─".repeat(60));

    for port_id in 1..=trunk.port_num {
        let type_key = format!("Port_{}", port_id);
        let grp_key = format!("Port_{}_grpInd", port_id);
        let state_key = format!("Port_{}_state", port_id);

        let ptype = raw
            .get(&type_key)
            .and_then(|v| v.get(format!("portTypeId_{}", port_id)))
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        let group = raw.get(&grp_key).and_then(|v| v.as_u64()).unwrap_or(0);
        let state = raw.get(&state_key).and_then(|v| v.as_u64()).unwrap_or(0);

        let type_str = match ptype {
            0 => pad_str("Static", 12),
            1 => pad_str("LACP", 12),
            _ => pad_str("Other", 12),
        };

        let group_str: String = if group == 0 && state == 0 {
            pad_str("─", 12).dimmed().to_string()
        } else {
            pad_str(format!("Group {}", group), 12).green().to_string()
        };

        let port_str = pad_str(format!("Port {}", port_id), 7).bold().to_string();
        println!(" {} {} {}", port_str, type_str, group_str);
    }

    Ok(())
}

fn cmd_vlan(client: &ApiClient, json: bool) -> Result<()> {
    let vlan = client.get_port_vlan()?;
    let pvids = client.get_all_port_pvids().ok();

    if json {
        println!("{}", serde_json::to_string_pretty(&vlan)?);
        return Ok(());
    }

    println!(" {}", "PORT VLAN CONFIGURATION".bold().white());
    println!("{}", "╌".repeat(50));
    println!(
        " {} {} {}",
        pad_str("Port", 7),
        pad_str("PVID", 8),
        pad_str("Frame Type", 12),
    );
    println!("{}", "─".repeat(50));

    for p in vlan.ports() {
        let frame_str = match p.frame_type {
            0 => "All",
            1 => "Tagged",
            2 => "Untagged",
            _ => "Unknown",
        };
        let port_str = pad_str(format!("Port {}", p.port_id), 7).bold().to_string();
        println!(
            " {} {} {}",
            port_str,
            pad_str(p.pvid, 8),
            pad_str(frame_str, 12)
        );
    }

    if let Some(pvids) = pvids {
        println!();
        println!(" {} (from compact endpoint)", "PVIDs:".dimmed());
        println!("   {:?}", pvids.port_pvids);
    }

    Ok(())
}

fn cmd_stp(client: &ApiClient, json: bool) -> Result<()> {
    let stp = client.get_stp_config()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&stp)?);
        return Ok(());
    }

    let enabled_str = if stp.stp_enable == "1" {
        "Enabled".green()
    } else {
        "Disabled".yellow()
    };

    println!(
        " {}  Mode: {}  ({})",
        "SPANNING TREE".bold().white(),
        stp.stp_rstp_mode,
        enabled_str
    );
    println!("{}", "╌".repeat(60));
    println!(
        " {} {} {} {}",
        pad_str("Port", 7),
        pad_str("Status", 12),
        pad_str("Edge", 8),
        pad_str("Path", 8),
    );
    println!("{}", "─".repeat(60));

    let port_count: u32 = stp.num_ports.parse().unwrap_or(10);
    for port_id in 1..=port_count {
        let pfx = format!("Port_{}", port_id);
        // Extract per-port fields from the raw JSON flattened data
        let port_raw = stp.raw.get(pfx);
        let status = port_raw
            .and_then(|v| v.get(format!("Stp_Status_{port_id}")))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let edge = port_raw
            .and_then(|v| v.get(format!("Stp_Edge_{port_id}")))
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let path_cost = port_raw
            .and_then(|v| v.get(format!("Stp_PathCost_{port_id}")))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let status_str: String = if stp.stp_enable != "1" {
            pad_str("Disabled", 12).dimmed().to_string()
        } else {
            match status {
                "Forward" => pad_str(status, 12).green().to_string(),
                "Blocking" | "Listening" | "Learning" => pad_str(status, 12).yellow().to_string(),
                _ => pad_str(status, 12).red().to_string(),
            }
        };
        let edge_str: String = if edge == "1" {
            pad_str("Yes", 8).cyan().to_string()
        } else {
            pad_str("No", 8).dimmed().to_string()
        };
        let path_str = if path_cost.is_empty() {
            pad_str("─", 8).dimmed().to_string()
        } else {
            pad_str(path_cost, 8).to_string()
        };

        let port_str = pad_str(format!("Port {port_id}"), 7).bold().to_string();
        println!(" {} {} {} {}", port_str, status_str, edge_str, path_str);
    }

    Ok(())
}

fn cmd_loop(client: &ApiClient, json: bool) -> Result<()> {
    let loop_status = client.get_loop_status()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&loop_status)?);
        return Ok(());
    }

    let violations = loop_status.violations();

    println!(" {}", "LOOP PROTECTION".bold().white());
    println!("{}", "╌".repeat(50));

    if violations.is_empty() {
        println!("  {} No loop violations detected", "✓".green());
    } else {
        for (port, status) in &violations {
            println!(
                "  {} Port {}: violation detected ({})",
                "✗".red(),
                port,
                status
            );
        }
    }

    Ok(())
}

fn cmd_igmp(client: &ApiClient, json: bool) -> Result<()> {
    let igmp = client.get_igmp_config()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&igmp)?);
        return Ok(());
    }

    println!(" {}", "IGMP SNOOPING".bold().white());
    println!("{}", "╌".repeat(50));

    let on_off = |v: &str| {
        if v == "on" {
            "On".green()
        } else {
            "Off".red()
        }
    };

    println!("  {:<20}  {}", "IGMP Snooping:", on_off(&igmp.igmp));
    println!("  {:<20}  {}", "Fast Leave:", on_off(&igmp.fast_leave));
    println!("  {:<20}  {}", "Report Flood:", on_off(&igmp.report_flood));

    Ok(())
}

fn cmd_storm(client: &ApiClient, json: bool) -> Result<()> {
    let storm = client.get_storm_control()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&storm)?);
        return Ok(());
    }

    println!(" {}", "STORM CONTROL".bold().white());
    println!("{}", "╌".repeat(70));

    let has_any = storm.ports.iter().any(|p| {
        p.sctrl_bcast > 0 || p.sctrl_mcast > 0 || p.sctrl_unucast > 0 || p.sctrl_unmcast > 0
    });

    if !has_any {
        println!("  {} All storm control disabled", "—".dimmed());
    } else {
        println!(
            " {} {} {} {} {}",
            pad_str("Port", 7),
            pad_str("Broadcast", 12),
            pad_str("Multicast", 12),
            pad_str("Unicast", 12),
            pad_str("UnMcast", 12),
        );
        println!("{}", "─".repeat(70));
        for p in &storm.ports {
            let port_str = pad_str(format!("Port {}", p.port_id), 7).bold().to_string();
            let bcast: String = if p.sctrl_bcast > 0 {
                pad_str(p.sctrl_bcast, 12).yellow().to_string()
            } else {
                pad_str("0", 12).dimmed().to_string()
            };
            let mcast: String = if p.sctrl_mcast > 0 {
                pad_str(p.sctrl_mcast, 12).yellow().to_string()
            } else {
                pad_str("0", 12).dimmed().to_string()
            };
            let ucast: String = if p.sctrl_unucast > 0 {
                pad_str(p.sctrl_unucast, 12).yellow().to_string()
            } else {
                pad_str("0", 12).dimmed().to_string()
            };
            let unmcast: String = if p.sctrl_unmcast > 0 {
                pad_str(p.sctrl_unmcast, 12).yellow().to_string()
            } else {
                pad_str("0", 12).dimmed().to_string()
            };
            println!(" {} {} {} {} {}", port_str, bcast, mcast, ucast, unmcast,);
        }
    }

    Ok(())
}

fn cmd_mirror(client: &ApiClient, json: bool) -> Result<()> {
    let mirror = client.get_port_mirror()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&mirror)?);
        return Ok(());
    }

    println!(" {}", "PORT MIRRORING".bold().white());
    println!("{}", "╌".repeat(70));

    if mirror.monitoring_port_id == "0" {
        println!("  {} Mirroring disabled", "—".dimmed());
    } else {
        println!("  Monitoring Port: {}", mirror.monitoring_port_id);
        println!(
            " {} {} {}",
            pad_str("Port", 7),
            pad_str("Ingress", 14),
            pad_str("Egress", 14),
        );
        println!("{}", "─".repeat(70));

        let entries = vec![
            (&mirror.port_1, 1),
            (&mirror.port_2, 2),
            (&mirror.port_3, 3),
            (&mirror.port_4, 4),
            (&mirror.port_5, 5),
            (&mirror.port_6, 6),
            (&mirror.port_7, 7),
            (&mirror.port_8, 8),
            (&mirror.port_9, 9),
            (&mirror.port_10, 10),
        ];

        for (entry, id) in &entries {
            let port_str = pad_str(format!("Port {}", id), 7).bold().to_string();
            let in_str: String = if entry.ingress_status == "Enabled" {
                pad_str("Enabled", 14).green().to_string()
            } else {
                pad_str("Disabled", 14).dimmed().to_string()
            };
            let eg_str: String = if entry.egress_status == "Enabled" {
                pad_str("Enabled", 14).green().to_string()
            } else {
                pad_str("Disabled", 14).dimmed().to_string()
            };
            println!(" {} {} {}", port_str, in_str, eg_str);
        }
    }

    Ok(())
}

fn cmd_network(client: &ApiClient, json: bool) -> Result<()> {
    let net = client.get_network_settings()?;

    if json {
        println!("{}", serde_json::to_string_pretty(&net)?);
        return Ok(());
    }

    println!(" {}", "NETWORK SETTINGS".bold().white());
    println!("{}", "╌".repeat(50));
    println!("  {:<18}  {}", "IP Address:", net.ip_address);
    println!("  {:<18}  {}", "Netmask:", net.netmask);
    println!("  {:<18}  {}", "Gateway:", net.gateway);
    println!("  {:<18}  {}", "DNS Server:", net.dns_server);
    println!(
        "  {:<18}  {}",
        "DHCP:",
        if net.dhcp_enabled == "1" {
            "Enabled".green()
        } else {
            "Static".blue()
        }
    );
    println!(
        "  {:<18}  {}",
        "Auto DNS:",
        if net.auto_dns_enabled == "1" {
            "Enabled".green()
        } else {
            "Disabled".yellow()
        }
    );

    Ok(())
}

fn cmd_all(client: &ApiClient, json: bool) -> Result<()> {
    cmd_status(client, json)?;
    println!();
    cmd_ports(client, json)?;
    println!();
    cmd_statistics(client, json, false)?;
    println!();
    cmd_mac(client, json)?;
    println!();
    cmd_vlan(client, json)?;
    println!();
    cmd_network(client, json)?;
    println!();
    cmd_loop(client, json)?;
    println!();
    cmd_stp(client, json)?;
    Ok(())
}

// ===========================================================================
// Write command implementations
// ===========================================================================

fn cmd_port_set(
    client: &ApiClient,
    json: bool,
    port: u32,
    status: Option<&str>,
    speed: Option<&str>,
    flow: Option<&str>,
) -> Result<()> {
    let mut settings = PortSettingsRequest::default();

    if let Some(s) = status {
        settings.port_status = Some(match s {
            "enabled" => "Enabled".to_string(),
            "disabled" => "Disabled".to_string(),
            _ => anyhow::bail!("Invalid status: {}. Use 'enabled' or 'disabled'.", s),
        });
    }
    if let Some(s) = speed {
        settings.spd_duplex_cfg = Some(match s {
            "auto" => "Auto".to_string(),
            "100" => "100MbpsFull".to_string(),
            "1000" => "1000MbpsFull".to_string(),
            "2500" => "2500MbpsFull".to_string(),
            _ => anyhow::bail!("Invalid speed: {}", s),
        });
    }
    if let Some(f) = flow {
        settings.flow_ctrl_cfg = Some(match f {
            "on" => "On".to_string(),
            "off" => "Off".to_string(),
            _ => anyhow::bail!("Invalid flow control: {}", f),
        });
    }

    let request = PortSettingsApplyRequest::single_port(port, settings)
        .context("Failed to build port settings request")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Port {}: applying {}",
            "◌ MOCK".yellow(),
            port.to_string().cyan(),
            describe_port_changes(status, speed, flow)
        );
    }

    let resp = client.set_port_settings(&request)?;
    if resp.is_success() {
        println!("  {} Port {} settings updated", "✓".green(), port);
        if client.is_mock() {
            println!(
                "  {} Changes are in mock only. Use --apply to write to real switch.",
                "ℹ".dimmed()
            );
        }
    } else {
        println!("  {} Failed to update port settings", "✗".red());
    }
    Ok(())
}

fn describe_port_changes(status: Option<&str>, speed: Option<&str>, flow: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(s) = status {
        parts.push(format!("status={}", s));
    }
    if let Some(s) = speed {
        parts.push(format!("speed={}", s));
    }
    if let Some(f) = flow {
        parts.push(format!("flow={}", f));
    }
    if parts.is_empty() {
        "no changes".to_string()
    } else {
        parts.join(", ")
    }
}

fn cmd_description_set(client: &ApiClient, json: bool, text: &str) -> Result<()> {
    if json {
        let request = SetDescriptionRequest {
            des: text.to_string(),
        };
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Description: '{}' → '{}'",
            "◌ MOCK".yellow(),
            client.get_system_info()?.des,
            text
        );
    }

    let resp = client.set_description(text)?;
    if resp.is_success() {
        println!("  {} Description updated to '{}'", "✓".green(), text);
    } else {
        println!("  {} Failed to update description", "✗".red());
    }
    Ok(())
}

fn cmd_vlan_set(
    client: &ApiClient,
    json: bool,
    port: u32,
    pvid: Option<u32>,
    frame_type: Option<&str>,
) -> Result<()> {
    let pvid = pvid.unwrap_or(1);
    let ft = match frame_type.unwrap_or("all") {
        "all" => 0u32,
        "tagged" => 1,
        "untagged" => 2,
        _ => anyhow::bail!("Invalid frame type. Use 'all', 'tagged', or 'untagged'."),
    };

    let request =
        PortVlanRequest::single_port(port, pvid, ft).context("Failed to build VLAN request")?;

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Port {}: PVID={}, FrameType={}",
            "◌ MOCK".yellow(),
            port,
            pvid,
            frame_type.unwrap_or("all")
        );
    }

    let resp = client.set_port_vlan(&request)?;
    if resp.is_success() {
        println!("  {} VLAN settings updated for Port {}", "✓".green(), port);
    } else {
        println!("  {} Failed to update VLAN", "✗".red());
    }
    Ok(())
}

fn cmd_igmp_set(
    client: &ApiClient,
    json: bool,
    snooping: Option<&str>,
    fast_leave: Option<&str>,
    report_flood: Option<&str>,
) -> Result<()> {
    let request = IgmpConfigRequest {
        igmp: snooping.map(|s| s.to_string()),
        fast_leave: fast_leave.map(|s| s.to_string()),
        report_flood: report_flood.map(|s| s.to_string()),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} IGMP: snooping={:?}, fast_leave={:?}, report_flood={:?}",
            "◌ MOCK".yellow(),
            snooping,
            fast_leave,
            report_flood
        );
    }

    let resp = client.set_igmp_config(&request)?;
    if resp.is_success() {
        println!("  {} IGMP config updated", "✓".green());
    } else {
        println!("  {} Failed to update IGMP config", "✗".red());
    }
    Ok(())
}

fn cmd_network_set(
    client: &ApiClient,
    json: bool,
    ip: Option<&str>,
    mask: Option<&str>,
    gateway: Option<&str>,
    dns: Option<&str>,
    dhcp: Option<&str>,
) -> Result<()> {
    let request = NetworkSettingsRequest {
        ip_address: ip.map(|s| s.to_string()),
        netmask: mask.map(|s| s.to_string()),
        gateway: gateway.map(|s| s.to_string()),
        dns_server: dns.map(|s| s.to_string()),
        dhcp_enabled: dhcp.map(|s| {
            if s == "on" {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }),
        auto_dns_enabled: dhcp.map(|s| {
            if s == "on" {
                "1".to_string()
            } else {
                "0".to_string()
            }
        }),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Network: ip={:?}, mask={:?}, gateway={:?}, dns={:?}, dhcp={:?}",
            "◌ MOCK".yellow(),
            ip,
            mask,
            gateway,
            dns,
            dhcp
        );
    }

    let resp = client.set_network_settings(&request)?;
    if resp.is_success() {
        println!("  {} Network settings updated", "✓".green());
    } else {
        println!("  {} Failed to update network settings", "✗".red());
    }
    Ok(())
}

fn cmd_storm_set(
    client: &ApiClient,
    json: bool,
    port: u32,
    broadcast: u32,
    multicast: u32,
    unicast: u32,
    unmcast: u32,
) -> Result<()> {
    if !(1..=10).contains(&port) {
        anyhow::bail!("Invalid port number: {}. Must be 1–10.", port);
    }
    let request = StormControlRequest {
        portnum: 10,
        ports: vec![StormControlPortRequest {
            port_id: port,
            sctrl_bcast: broadcast,
            sctrl_mcast: multicast,
            sctrl_unucast: unicast,
            sctrl_unmcast: unmcast,
        }],
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Port {} storm control: bcast={}, mcast={}, ucast={}, unmcast={} (Kbps)",
            "◌ MOCK".yellow(),
            port,
            broadcast,
            multicast,
            unicast,
            unmcast
        );
    }

    let resp = client.set_storm_control(&request)?;
    if resp.is_success() {
        println!("  {} Storm control updated for Port {}", "✓".green(), port);
    } else {
        println!("  {} Failed to update storm control", "✗".red());
    }
    Ok(())
}

fn cmd_stp_set(client: &ApiClient, json: bool, action: &str) -> Result<()> {
    let enable = match action {
        "enable" => true,
        "disable" => false,
        _ => anyhow::bail!(
            "Invalid STP action: '{}'. Use 'enable' or 'disable'.",
            action
        ),
    };
    let request = if enable {
        StpConfigRequest::enable()
    } else {
        StpConfigRequest::disable()
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} STP: {}",
            "◌ MOCK".yellow(),
            if enable { "enable" } else { "disable" }
        );
    }

    let resp = client.set_stp_config(&request)?;
    if resp.is_success() {
        println!(
            "  {} STP {}",
            "✓".green(),
            if enable { "enabled" } else { "disabled" }
        );
    } else {
        println!("  {} Failed to update STP config", "✗".red());
    }
    Ok(())
}

fn cmd_statistics_clear(client: &ApiClient, json: bool) -> Result<()> {
    if json {
        println!("{{\"command\":\"clear-statistics\"}}");
        return Ok(());
    }

    if client.is_mock() {
        println!(" {} Clearing port statistics", "◌ MOCK".yellow());
    }

    let resp = client.clear_statistics()?;
    if resp.is_success() {
        println!("  {} Port statistics cleared", "✓".green());
    } else {
        println!("  {} Failed to clear statistics", "✗".red());
    }
    Ok(())
}

fn cmd_mac_clear(client: &ApiClient, json: bool) -> Result<()> {
    if json {
        println!("{{\"command\":\"clear-mac\"}}");
        return Ok(());
    }

    if client.is_mock() {
        let count = client
            .get_dynamic_mac_entries()
            .map(|e| e.len())
            .unwrap_or(0);
        println!(
            " {} Clearing {} dynamic MAC entries",
            "◌ MOCK".yellow(),
            count
        );
    }

    let resp = client.clear_mac_entries()?;
    if resp.is_success() {
        println!("  {} MAC table cleared", "✓".green());
    } else {
        println!("  {} Failed to clear MAC table", "✗".red());
    }
    Ok(())
}

fn cmd_config_save(client: &ApiClient, json: bool) -> Result<()> {
    if json {
        println!("{{\"command\":\"save-config\"}}");
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Saving configuration (mock — no real switch affected)",
            "◌ MOCK".yellow()
        );
    }

    let resp = client.save_config()?;
    if resp.is_success() {
        println!("  {} Running config saved to startup", "✓".green());
    } else {
        println!("  {} Failed to save config", "✗".red());
    }
    Ok(())
}

fn cmd_backup(client: &ApiClient, json: bool, output: Option<&str>) -> Result<()> {
    let backup = client.backup_config()?;

    if json {
        // Already JSON, print as-is
        println!("{}", backup);
        return Ok(());
    }

    match output {
        Some(path) => {
            std::fs::write(path, &backup)
                .with_context(|| format!("Failed to write backup to {}", path))?;
            println!("  {} Config backup saved to {}", "✓".green(), path);
            println!("  {} File size: {} bytes", "ℹ".dimmed(), backup.len());
        }
        None => {
            println!("{}", backup);
            println!();
            println!(
                "  {} Use --output <file> to save backup to a file",
                "ℹ".dimmed()
            );
        }
    }
    Ok(())
}

fn cmd_restore(client: &ApiClient, json: bool, file: &str, yes: bool) -> Result<()> {
    // Read backup file
    let data = std::fs::read_to_string(file)
        .with_context(|| format!("Failed to read backup file: {}", file))?;

    // Validate it's parseable JSON
    let _snapshot: SwitchConfigSnapshot =
        serde_json::from_str(&data).context("Invalid backup file: not a valid config snapshot")?;

    if json {
        println!("{}", data);
        return Ok(());
    }

    // Confirm for real switches
    if client.is_real() && !yes {
        use std::io::{self, Write};
        eprintln!(
            "{} This will overwrite the switch configuration with the backup.",
            "⚠".yellow()
        );
        eprint!("Type 'yes' to continue: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            println!("{} Restore cancelled", "✗".yellow());
            return Ok(());
        }
    }

    let resp = client.restore_config(&data)?;
    if resp.is_success() {
        println!("  {} Configuration restored from {}", "✓".green(), file);
        if client.is_mock() {
            println!("  {} Changes applied to mock only.", "ℹ".dimmed());
        }
    } else {
        println!("  {} Failed to restore configuration", "✗".red());
    }
    Ok(())
}

fn cmd_reboot(client: &ApiClient, json: bool, yes: bool) -> Result<()> {
    if json {
        println!("{{\"command\":\"reboot\"}}");
        return Ok(());
    }

    if client.is_real() && !yes {
        use std::io::{self, Write};
        eprintln!(
            "{} This will REBOOT the switch. It will be unreachable for ~30 seconds.",
            "⚠".yellow()
        );
        eprint!("Type 'yes' to continue: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            println!("{} Reboot cancelled", "✗".yellow());
            return Ok(());
        }
    }

    if client.is_mock() {
        println!(
            " {} Mock reboot (no real switch affected)",
            "◌ MOCK".yellow()
        );
    }

    let resp = client.reboot()?;
    if resp.is_success() {
        println!("  {} Reboot initiated", "✓".green());
    } else {
        println!("  {} Failed to reboot", "✗".red());
    }
    Ok(())
}

fn cmd_factory_reset(client: &ApiClient, json: bool, yes: bool) -> Result<()> {
    if json {
        println!("{{\"command\":\"factory-reset\"}}");
        return Ok(());
    }

    if client.is_real() && !yes {
        use std::io::{self, Write};
        eprintln!(
            "{} FACTORY RESET will DESTROY ALL CONFIGURATION on the switch.",
            "⚠".red().bold()
        );
        eprint!("Type 'yes' to continue: ");
        io::stdout().flush().ok();
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if input.trim() != "yes" {
            println!("{} Factory reset cancelled", "✗".yellow());
            return Ok(());
        }
    }

    if client.is_mock() {
        println!(
            " {} Mock factory reset (no real switch affected)",
            "◌ MOCK".yellow()
        );
    }

    let resp = client.factory_reset()?;
    if resp.is_success() {
        println!("  {} Factory reset initiated", "✓".green());
    } else {
        println!("  {} Failed to factory reset", "✗".red());
    }
    Ok(())
}

fn cmd_mirror_set(
    client: &ApiClient,
    json: bool,
    monitor_port: &Option<u32>,
    source_ingress: Option<&str>,
    source_egress: Option<&str>,
) -> Result<()> {
    let monitor_port_id = monitor_port
        .map(|p| p.to_string())
        .unwrap_or_else(|| "0".to_string());

    let mut ingress_ports: Vec<u32> = Vec::new();
    let mut egress_ports: Vec<u32> = Vec::new();

    if let Some(ports) = source_ingress {
        for p in ports.split(',') {
            let p = p.trim();
            if !p.is_empty() {
                ingress_ports.push(p.parse().context("Invalid port number in source_ingress")?);
            }
        }
    }

    if let Some(ports) = source_egress {
        for p in ports.split(',') {
            let p = p.trim();
            if !p.is_empty() {
                egress_ports.push(p.parse().context("Invalid port number in source_egress")?);
            }
        }
    }

    let mut request = PortMirrorRequest {
        port_num: "10".to_string(),
        monitoring_port_id: monitor_port_id.clone(),
        port_1: None,
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

    for port_id in 1..=10u32 {
        let has_ingress = ingress_ports.contains(&port_id);
        let has_egress = egress_ports.contains(&port_id);

        if has_ingress || has_egress {
            let entry = PortMirrorEntry {
                port_id: port_id.to_string(),
                ingress_status: if has_ingress {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
                egress_status: if has_egress {
                    "Enabled".to_string()
                } else {
                    "Disabled".to_string()
                },
            };
            set_mirror_entry(&mut request, port_id, Some(entry))?;
        }
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Port mirror: monitor={}, ingress={:?}, egress={:?}",
            "◌ MOCK".yellow(),
            monitor_port_id,
            source_ingress,
            source_egress,
        );
    }

    let resp = client.set_port_mirror(&request)?;
    if resp.is_success() {
        println!("  {} Port mirror configuration updated", "✓".green());
    } else {
        println!("  {} Failed to update port mirror", "✗".red());
    }
    Ok(())
}

/// Set a port entry on a PortMirrorRequest by port number.
fn set_mirror_entry(
    request: &mut PortMirrorRequest,
    port_id: u32,
    entry: Option<PortMirrorEntry>,
) -> Result<()> {
    match port_id {
        1 => request.port_1 = entry,
        2 => request.port_2 = entry,
        3 => request.port_3 = entry,
        4 => request.port_4 = entry,
        5 => request.port_5 = entry,
        6 => request.port_6 = entry,
        7 => request.port_7 = entry,
        8 => request.port_8 = entry,
        9 => request.port_9 = entry,
        10 => request.port_10 = entry,
        _ => anyhow::bail!("Invalid port number: {}. Must be 1-10.", port_id),
    }
    Ok(())
}

fn cmd_trunk_set(
    client: &ApiClient,
    json: bool,
    port: u32,
    trunk_type: Option<&str>,
    group: Option<u32>,
) -> Result<()> {
    let mapped_type = trunk_type.map(|t| match t {
        "static" => 0u32,
        "lacp" => 1u32,
        "none" => 2u32,
        _ => unreachable!(), // value_parser restricts to ["static", "lacp", "none"]
    });

    let state = mapped_type.map(|t| if t == 2 { 0u32 } else { 1u32 });

    let entry = TrunkPortEntry {
        port_type: mapped_type,
        port_priority: None,
        lacp_timeout: None,
        group_index: group,
        state,
    };

    let mut request = TrunkConfigRequest {
        port_num: 10,
        system_priority: 32768,
        port_1: None,
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

    // Set the entry for the specified port
    match port {
        1 => request.port_1 = Some(entry),
        2 => request.port_2 = Some(entry),
        3 => request.port_3 = Some(entry),
        4 => request.port_4 = Some(entry),
        5 => request.port_5 = Some(entry),
        6 => request.port_6 = Some(entry),
        7 => request.port_7 = Some(entry),
        8 => request.port_8 = Some(entry),
        9 => request.port_9 = Some(entry),
        10 => request.port_10 = Some(entry),
        _ => anyhow::bail!("Invalid port number: {}. Must be 1-10.", port),
    }

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Trunk: port={}, type={:?}, group={:?}",
            "◌ MOCK".yellow(),
            port,
            trunk_type,
            group,
        );
    }

    let resp = client.set_trunk_config(&request)?;
    if resp.is_success() {
        println!("  {} Trunk config updated for Port {}", "✓".green(), port);
    } else {
        println!("  {} Failed to update trunk config", "✗".red());
    }
    Ok(())
}

fn cmd_loop_protection_set(
    client: &ApiClient,
    json: bool,
    enable: Option<&str>,
    interval: Option<u32>,
    recover: Option<u32>,
) -> Result<()> {
    let detect_enable = enable.map(|e| {
        if e == "true" || e == "1" {
            "1".to_string()
        } else {
            "0".to_string()
        }
    });
    let request = LoopProtectionRequest {
        port_num: Some("10".to_string()),
        detect_enable,
        time_interval: interval.map(|n| n.to_string()),
        recover_time: recover.map(|n| n.to_string()),
    };

    if json {
        println!("{}", serde_json::to_string_pretty(&request)?);
        return Ok(());
    }

    if client.is_mock() {
        println!(
            " {} Loop protection: enable={:?}, interval={:?}, recover={:?}",
            "◌ MOCK".yellow(),
            enable,
            interval,
            recover,
        );
    }

    let resp = client.set_loop_protection(&request)?;
    if resp.is_success() {
        println!("  {} Loop protection configuration updated", "✓".green());
    } else {
        println!("  {} Failed to update loop protection", "✗".red());
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Formatting helpers
// ---------------------------------------------------------------------------

fn pad_str(text: impl std::fmt::Display, width: usize) -> String {
    format!("{:<width$}", text, width = width)
}

fn format_num(s: &str) -> String {
    let n: u64 = s.parse().unwrap_or(0);
    if n >= 1_000_000_000 {
        format!("{:.1}B", n as f64 / 1_000_000_000.0)
    } else if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}
