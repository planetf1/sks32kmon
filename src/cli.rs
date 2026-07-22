use std::path::Path;

use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::*;

use crate::client::SwitchClient;
use crate::config::{self, SwitchTarget};
#[cfg(feature = "tui")]
use crate::tui;

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

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
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
    /// TUI dashboard (live monitoring; Tab to switch between devices)
    #[cfg(feature = "tui")]
    Monitor,
    /// Generate a sample config file template (prints to stdout)
    ConfigInit,
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
        Command::Monitor => {
            tui::run_tui(&targets)?;
        }
        cmd => {
            for (i, target) in targets.iter().enumerate() {
                if i > 0 {
                    println!();
                }
                let client = SwitchClient::connect(&target.host, &target.user, &target.password)?;
                run_command_on(cmd, &client, args.json, target)?;
            }
        }
    }

    Ok(())
}

/// Run a single command against one connected switch.
fn run_command_on(
    cmd: &Command,
    client: &SwitchClient,
    json: bool,
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
        Command::Status => cmd_status(client, json),
        Command::Ports => cmd_ports(client, json),
        Command::Statistics { watch } => cmd_statistics(client, json, *watch),
        Command::Mac => cmd_mac(client, json),
        Command::StaticMac => cmd_static_mac(client, json),
        Command::Trunk => cmd_trunk(client, json),
        Command::Vlan => cmd_vlan(client, json),
        Command::Stp => cmd_stp(client, json),
        Command::Loop => cmd_loop(client, json),
        Command::Igmp => cmd_igmp(client, json),
        Command::Storm => cmd_storm(client, json),
        Command::Mirror => cmd_mirror(client, json),
        Command::Network => cmd_network(client, json),
        Command::All => cmd_all(client, json),
        Command::ConfigInit => unreachable!(), // handled in run()
        #[cfg(feature = "tui")]
        Command::Monitor => unreachable!(), // handled in run()
    }
}

// ===========================================================================
// Command implementation functions (unchanged — each takes one SwitchClient)
// ===========================================================================

fn cmd_status(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_ports(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_statistics(client: &SwitchClient, json: bool, watch: bool) -> Result<()> {
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

fn cmd_mac(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_static_mac(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_trunk(client: &SwitchClient, json: bool) -> Result<()> {
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
            .and_then(|v| v.get(&format!("portTypeId_{}", port_id)))
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

fn cmd_vlan(client: &SwitchClient, json: bool) -> Result<()> {
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
            pad_str(&p.pvid, 8),
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

fn cmd_stp(client: &SwitchClient, json: bool) -> Result<()> {
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
        let port_raw = stp.raw.get(&pfx);
        let status = port_raw
            .and_then(|v| v.get(&format!("Stp_Status_{port_id}")))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");
        let edge = port_raw
            .and_then(|v| v.get(&format!("Stp_Edge_{port_id}")))
            .and_then(|v| v.as_str())
            .unwrap_or("0");
        let path_cost = port_raw
            .and_then(|v| v.get(&format!("Stp_PathCost_{port_id}")))
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

fn cmd_loop(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_igmp(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_storm(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_mirror(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_network(client: &SwitchClient, json: bool) -> Result<()> {
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

fn cmd_all(client: &SwitchClient, json: bool) -> Result<()> {
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
