use std::collections::VecDeque;
use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui::Terminal;

use crate::api::ApiClient;
use crate::config::SwitchTarget;
use crate::models::*;
use crate::write_models::*;

// ---------------------------------------------------------------------------
// Per-switch data snapshot
// ---------------------------------------------------------------------------

struct SwitchData {
    target: SwitchTarget,
    client: ApiClient,
    system_info: Option<SystemInfo>,
    port_settings: Option<PortSettingsResponse>,
    port_stats: Option<PortStatisticsResponse>,
    prev_port_stats: Option<PortStatisticsResponse>,
    prev_stats_time: Option<Instant>,
    mac_entries: Option<Vec<MacEntry>>,
    last_refresh: Option<chrono::DateTime<Local>>,
    error: Option<String>,
    // Pre-computed per-port rates from the last refresh
    port_rates: Vec<PortRateSnapshot>,
    // Ring buffers for sparkline history (one per port)
    port_histories: Vec<PortHistory>,
    // MAC churn tracking
    prev_mac_count: Option<usize>,
    mac_churn: u64,
    // Cached config data (fetched once per refresh, not on every render)
    network_settings: Option<NetworkSettings>,
    port_vlan: Option<PortVlanResponse>,
    stp_config: Option<StpConfig>,
    igmp_config: Option<IgmpConfig>,
    storm_control: Option<StormControlResponse>,
    port_mirror: Option<PortMirrorResponse>,
    trunk_config: Option<TrunkConfigResponse>,
}

impl SwitchData {
    fn new(target: SwitchTarget, mock: bool) -> Result<Self> {
        let client = if mock {
            ApiClient::connect_mock(&target.host, &target.user, &target.password)
                .with_context(|| format!("Failed to init mock client for {}", target.host))?
        } else {
            ApiClient::connect_real(&target.host, &target.user, &target.password)
                .with_context(|| format!("Failed to connect to {}", target.host))?
        };
        Ok(Self {
            target,
            client,
            system_info: None,
            port_settings: None,
            port_stats: None,
            prev_port_stats: None,
            prev_stats_time: None,
            mac_entries: None,
            last_refresh: None,
            error: None,
            port_rates: vec![PortRateSnapshot::default(); 10],
            port_histories: (0..10).map(|_| PortHistory::new()).collect(),
            prev_mac_count: None,
            mac_churn: 0,
            network_settings: None,
            port_vlan: None,
            stp_config: None,
            igmp_config: None,
            storm_control: None,
            port_mirror: None,
            trunk_config: None,
        })
    }

    fn refresh(&mut self) {
        let now = Instant::now();

        match self.client.get_system_info() {
            Ok(info) => {
                self.system_info = Some(info);
                self.error = None;
            }
            Err(e) => {
                self.error = Some(format!("Status: {}", e));
                return;
            }
        }

        match self.client.get_port_settings() {
            Ok(ports) => self.port_settings = Some(ports),
            Err(e) => {
                self.error = Some(format!("Port settings: {}", e));
                return;
            }
        }

        match self.client.get_port_statistics() {
            Ok(stats) => {
                // Compute per-port rates from counter deltas
                let elapsed = self
                    .prev_stats_time
                    .map(|t| now.duration_since(t).as_secs_f64().max(0.1))
                    .unwrap_or(1.0);

                let prev_ports = self.prev_port_stats.as_ref().map(|p| p.ports());

                if let Some(prev_ports) = prev_ports {
                    for (i, p) in stats.ports().iter().enumerate() {
                        let prev = prev_ports.get(i);
                        let tx: f64 = p.tx_good_pkt.parse().unwrap_or(0.0);
                        let rx: f64 = p.rx_good_pkt.parse().unwrap_or(0.0);
                        let tx_bad: f64 = p.tx_bad_pkt.parse().unwrap_or(0.0);
                        let rx_bad: f64 = p.rx_bad_pkt.parse().unwrap_or(0.0);

                        if let Some(pp) = prev {
                            let ptx: f64 = pp.tx_good_pkt.parse().unwrap_or(0.0);
                            let prx: f64 = pp.rx_good_pkt.parse().unwrap_or(0.0);
                            let ptx_bad: f64 = pp.tx_bad_pkt.parse().unwrap_or(0.0);
                            let prx_bad: f64 = pp.rx_bad_pkt.parse().unwrap_or(0.0);

                            let tx_rate = ((tx - ptx).max(0.0) / elapsed) as u64;
                            let rx_rate = ((rx - prx).max(0.0) / elapsed) as u64;
                            let tx_err = ((tx_bad - ptx_bad).max(0.0) / elapsed) as u64;
                            let rx_err = ((rx_bad - prx_bad).max(0.0) / elapsed) as u64;

                            self.port_rates[i] = PortRateSnapshot {
                                tx_rate,
                                rx_rate,
                                tx_err_rate: tx_err,
                                rx_err_rate: rx_err,
                            };

                            // Push total good + bad rate to sparkline history
                            self.port_histories[i].push(tx_rate + rx_rate + tx_err + rx_err);
                        } else {
                            // New port appeared (shouldn't normally happen)
                            self.port_rates[i] = PortRateSnapshot::default();
                            self.port_histories[i].push(0);
                        }
                    }
                } else {
                    // First refresh — no deltas yet, push zeros
                    for (i, _p) in stats.ports().iter().enumerate() {
                        self.port_rates[i] = PortRateSnapshot::default();
                        self.port_histories[i].push(0);
                    }
                }

                self.prev_port_stats = self.port_stats.take();
                self.prev_stats_time = Some(now);
                self.port_stats = Some(stats);
            }
            Err(e) => {
                self.error = Some(format!("Port stats: {}", e));
                return;
            }
        }

        match self.client.get_dynamic_mac_entries() {
            Ok(entries) => {
                // Track MAC churn: absolute change in entry count
                let new_count = entries.len();
                if let Some(prev) = self.prev_mac_count {
                    self.mac_churn = (new_count as i64 - prev as i64).unsigned_abs();
                } else {
                    self.mac_churn = 0;
                }
                self.prev_mac_count = Some(new_count);
                self.mac_entries = Some(entries);
            }
            Err(e) => {
                self.error = Some(format!("MAC: {}", e));
            }
        }

        // Config pane caches — non-fatal (Config pane just shows stale data on error)
        self.network_settings = self.client.get_network_settings().ok();
        self.port_vlan = self.client.get_port_vlan().ok();
        self.stp_config = self.client.get_stp_config().ok();
        self.igmp_config = self.client.get_igmp_config().ok();
        self.storm_control = self.client.get_storm_control().ok();
        self.port_mirror = self.client.get_port_mirror().ok();
        self.trunk_config = self.client.get_trunk_config().ok();

        self.last_refresh = Some(chrono::Local::now());
    }
}

// ---------------------------------------------------------------------------
// Per-port rate snapshot and history for sparklines
// ---------------------------------------------------------------------------

/// Maximum number of data points in the ring buffer (~3 min at 3s refresh).
const HISTORY_MAX: usize = 60;

/// Pre-computed per-port rates (packets/s) from the last refresh delta.
#[derive(Clone, Default)]
struct PortRateSnapshot {
    tx_rate: u64,
    rx_rate: u64,
    tx_err_rate: u64,
    rx_err_rate: u64,
}

/// Ring buffer tracking one metric per port for sparkline rendering.
struct PortHistory {
    ring: VecDeque<u64>,
}

impl PortHistory {
    fn new() -> Self {
        Self {
            ring: VecDeque::with_capacity(HISTORY_MAX),
        }
    }

    fn push(&mut self, value: u64) {
        if self.ring.len() >= HISTORY_MAX {
            self.ring.pop_front();
        }
        self.ring.push_back(value);
    }
}

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

struct TuiApp {
    switches: Vec<SwitchData>,
    current_switch: usize,
    scroll_offset_port: usize,
    scroll_offset_mac: usize,
    active_pane: usize, // 0 = port, 1 = MAC, 2 = Config
    refresh_interval: u64,
    // Config editing
    config_cursor: usize, // which field row (0-indexed) is selected
    config_scroll: usize, // scroll offset in config pane
    editing: bool,
    edit_buffer: String,
}

impl TuiApp {
    fn current(&self) -> &SwitchData {
        &self.switches[self.current_switch]
    }

    fn current_mut(&mut self) -> &mut SwitchData {
        &mut self.switches[self.current_switch]
    }

    fn refresh_all(&mut self) {
        for sw in &mut self.switches {
            sw.refresh();
        }
    }

    fn refresh_current(&mut self) {
        self.current_mut().refresh();
    }
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn run_tui(targets: &[SwitchTarget], mock: bool) -> Result<()> {
    if targets.is_empty() {
        anyhow::bail!("No switches to monitor");
    }

    crossterm::terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, crossterm::terminal::EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    terminal.clear()?;

    // Connect to all switches
    let mut switches = Vec::new();
    for target in targets {
        match SwitchData::new(target.clone(), mock) {
            Ok(sd) => switches.push(sd),
            Err(e) => {
                // Print error to stderr before entering TUI, but can't now in raw mode.
                // We'll still try to show what we can.
                eprintln!("Failed to connect to {}: {}", target.host, e);
            }
        }
    }

    if switches.is_empty() {
        anyhow::bail!("Could not connect to any switches");
    }

    let mut app = TuiApp {
        switches,
        current_switch: 0,
        scroll_offset_port: 0,
        scroll_offset_mac: 0,
        active_pane: 0,
        refresh_interval: 3,
        config_cursor: 0,
        config_scroll: 0,
        editing: false,
        edit_buffer: String::new(),
    };

    app.refresh_all();

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        // Poll for input at the current refresh interval
        if event::poll(Duration::from_secs(app.refresh_interval))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    // Edit mode input handling (field editing in Config pane)
                    if app.editing {
                        match key.code {
                            KeyCode::Esc => {
                                app.editing = false;
                                app.edit_buffer.clear();
                            }
                            KeyCode::Enter => {
                                // Commit edit
                                if let Err(e) = apply_config_field(&mut app) {
                                    app.switches[app.current_switch].error = Some(format!("{}", e));
                                }
                                app.editing = false;
                                app.edit_buffer.clear();
                                app.refresh_all();
                            }
                            KeyCode::Backspace => {
                                app.edit_buffer.pop();
                            }
                            KeyCode::Char(c) => {
                                app.edit_buffer.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => break,
                        KeyCode::Esc => break,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.refresh_all();
                        }
                        KeyCode::Tab => {
                            app.active_pane = (app.active_pane + 1) % 3;
                            app.scroll_offset_port = 0;
                            app.scroll_offset_mac = 0;
                            app.config_cursor = 0;
                            app.config_scroll = 0;
                        }
                        KeyCode::Right => {
                            if app.switches.len() > 1 {
                                app.current_switch = (app.current_switch + 1) % app.switches.len();
                            }
                            app.scroll_offset_port = 0;
                            app.scroll_offset_mac = 0;
                            app.config_cursor = 0;
                            app.config_scroll = 0;
                        }
                        KeyCode::Left => {
                            app.current_switch = if app.current_switch == 0 {
                                app.switches.len() - 1
                            } else {
                                app.current_switch - 1
                            };
                            app.scroll_offset_port = 0;
                            app.scroll_offset_mac = 0;
                        }
                        // Config pane navigation
                        KeyCode::Up if app.active_pane == 2 && app.config_cursor > 0 => {
                            app.config_cursor -= 1;
                            if app.config_cursor < app.config_scroll {
                                app.config_scroll = app.config_cursor;
                            }
                        }
                        KeyCode::Down if app.active_pane == 2 => {
                            app.config_cursor += 1;
                            let field_count = config_field_count(&app);
                            if app.config_cursor >= field_count {
                                app.config_cursor = field_count.saturating_sub(1);
                            }
                        }
                        // Port pane navigation
                        KeyCode::Up if app.active_pane == 0 => {
                            if app.scroll_offset_port > 0 {
                                app.scroll_offset_port -= 1;
                            }
                        }
                        KeyCode::Down if app.active_pane == 0 => {
                            app.scroll_offset_port += 1;
                        }
                        // MAC pane navigation
                        KeyCode::Up if app.active_pane == 1 => {
                            if app.scroll_offset_mac > 0 {
                                app.scroll_offset_mac -= 1;
                            }
                        }
                        KeyCode::Down if app.active_pane == 1 => {
                            app.scroll_offset_mac += 1;
                        }
                        KeyCode::Up | KeyCode::Down => {}
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            app.refresh_interval = app.refresh_interval.saturating_sub(1).max(1);
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            app.refresh_interval = (app.refresh_interval + 1).min(60);
                        }
                        // Config pane: Enter to edit, a to apply, s to save
                        KeyCode::Enter if app.active_pane == 2 => {
                            let field = config_field_at(&app, app.config_cursor);
                            if !field.is_header {
                                app.editing = true;
                                app.edit_buffer = field.value.clone();
                            }
                        }
                        KeyCode::Char('a') if app.active_pane == 2 => {
                            if let Err(e) = apply_all_config(&mut app) {
                                app.switches[app.current_switch].error =
                                    Some(format!("Apply: {}", e));
                            }
                            app.refresh_all();
                        }
                        KeyCode::Char('s') if app.active_pane == 2 => {
                            let client = &app.switches[app.current_switch].client;
                            if let Err(e) = client.save_config() {
                                app.switches[app.current_switch].error =
                                    Some(format!("Save: {}", e));
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else {
            // Timeout — auto-refresh current switch
            app.refresh_current();
        }
    }

    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        terminal.backend_mut(),
        crossterm::terminal::LeaveAlternateScreen
    )?;
    println!("monitor closed");
    Ok(())
}

// ---------------------------------------------------------------------------
// UI drawing
// ---------------------------------------------------------------------------

fn draw_ui(f: &mut Frame, app: &TuiApp) {
    let area = f.area();

    let constraints = [
        Constraint::Length(4),
        Constraint::Min(10),
        Constraint::Length(1),
    ];
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    render_header(f, app, chunks[0]);
    render_body(f, app, chunks[1]);
    render_footer(f, app, chunks[2]);
}

// ---------------------------------------------------------------------------
// Header
// ---------------------------------------------------------------------------

fn render_header(f: &mut Frame, app: &TuiApp, area: Rect) {
    let current = app.current();

    let info = match &current.system_info {
        Some(i) => {
            // Count active ports (link up + enabled)
            let active_count = current
                .port_stats
                .as_ref()
                .map(|s| {
                    s.ports()
                        .iter()
                        .filter(|p| p.link_status != "Link Down" && p.port_status == "Enabled")
                        .count()
                })
                .unwrap_or(0);
            let title = format!(
                " SKS3200-8E2X @ {}  |  FW: {}  HW: {}  MAC: {}  {}°C  |  {}  |  Up: {}/10",
                i.sys_ipv4, i.fw_ver, i.hw_ver, i.sys_macaddr, i.temperature, i.des, active_count
            );
            Span::styled(
                title,
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
            )
        }
        None => Span::styled(
            " Connecting... ",
            Style::default().fg(Color::White).bg(Color::Blue),
        ),
    };

    // Tab indicator: show switch name tags
    let mut tags = Vec::new();
    for (i, sw) in app.switches.iter().enumerate() {
        let label = if sw.target.name != sw.target.host {
            format!(" {} ", sw.target.name)
        } else {
            format!(" {} ", sw.target.host)
        };
        if i == app.current_switch {
            tags.push(Span::styled(
                label,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            tags.push(Span::styled(label, Style::default().fg(Color::DarkGray)));
        }
    }

    let mut line = Line::from(info);
    line.push_span(Span::raw("  "));
    line.extend(tags);

    let p = Paragraph::new(line).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::White).bg(Color::Blue)),
    );

    f.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Body: port pane + MAC pane
// ---------------------------------------------------------------------------

fn render_body(f: &mut Frame, app: &TuiApp, area: Rect) {
    if app.active_pane == 2 {
        // Full-width config/edit pane
        render_config_pane(f, app, area);
    } else if app.active_pane == 1 {
        // MAC-only pane
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(area);
        render_mac_pane(f, app, chunks[0]);
    } else {
        // Split port + MAC panes (default)
        let constraints = [Constraint::Percentage(55), Constraint::Percentage(45)];
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);
        render_port_pane(f, app, chunks[0]);
        render_mac_pane(f, app, chunks[1]);
    }
}

// ---------------------------------------------------------------------------
// Port pane
// ---------------------------------------------------------------------------

fn render_port_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let border_style = if app.active_pane == 0 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let current = app.current();
    let stats = match &current.port_stats {
        Some(s) => s,
        None => {
            let p = Paragraph::new("Waiting for data...").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Ports ")
                    .border_style(border_style),
            );
            f.render_widget(p, area);
            return;
        }
    };

    let port_data: Vec<(usize, &PortStats)> = stats.ports().into_iter().enumerate().collect();

    // Split area: table on top, sparklines on bottom (if there's room)
    let sparkline_height = if area.height >= 15 {
        // Show sparklines for up to 10 ports, but constrained by available space
        let active_ports = port_data
            .iter()
            .filter(|(_, p)| p.link_status != "Link Down" && p.port_status == "Enabled")
            .count();
        (active_ports.min(10) as u16).min(area.height.saturating_sub(14))
    } else {
        0
    };

    let (table_area, spark_area) = if sparkline_height > 0 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(10),
                Constraint::Length(sparkline_height + 2), // +2 for border
            ])
            .split(area);
        (chunks[0], Some(chunks[1]))
    } else {
        (area, None)
    };

    let max_visible = (table_area.height as usize).saturating_sub(3);
    let scroll = app
        .scroll_offset_port
        .min(port_data.len().saturating_sub(max_visible));

    // Expanded header with error rate columns
    let header_cells = [
        "Port", "Status", "Speed", "Tx/s", "Rx/s", "TxErr", "RxErr", "Err%",
    ]
    .iter()
    .map(|h| {
        Cell::from(Span::styled(
            *h,
            Style::default().add_modifier(Modifier::BOLD),
        ))
    });
    let header =
        Row::new(header_cells).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    // Pre-compute aggregate totals
    let mut agg_tx: u64 = 0;
    let mut agg_rx: u64 = 0;
    let mut agg_tx_err: u64 = 0;
    let mut agg_rx_err: u64 = 0;

    let rows: Vec<Row> = port_data
        .iter()
        .skip(scroll)
        .take(max_visible)
        .map(|(i, p)| {
            let port_num = i + 1;
            let status_style = if p.port_status == "Disabled" {
                Style::default().fg(Color::Red)
            } else if p.link_status == "Link Down" {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            };

            let status_str = if p.link_status != "Link Down" && p.port_status == "Enabled" {
                "Up"
            } else if p.port_status == "Disabled" {
                "Disabled"
            } else {
                "Down"
            };

            let speed_str = if p.link_status != "Link Down" {
                shorten_speed(&p.link_status)
            } else {
                "--"
            };

            // Use pre-computed rates from SwitchData
            let rates = &current.port_rates[*i];
            let tx_str = format_rate(rates.tx_rate);
            let rx_str = format_rate(rates.rx_rate);
            let tx_err_str = format_rate(rates.tx_err_rate);
            let rx_err_str = format_rate(rates.rx_err_rate);

            // Error ratio: bad / (good + bad) * 100
            let total_pkts = rates.tx_rate + rates.rx_rate + rates.tx_err_rate + rates.rx_err_rate;
            let total_errs = rates.tx_err_rate + rates.rx_err_rate;
            let err_pct = if total_pkts > 0 {
                (total_errs as f64 / total_pkts as f64) * 100.0
            } else {
                0.0
            };
            let err_str = if err_pct < 0.01 && total_pkts > 0 {
                "<0.01%".to_string()
            } else {
                format!("{:.2}%", err_pct)
            };

            let err_style = if err_pct > 1.0 {
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
            } else if err_pct > 0.1 {
                Style::default().fg(Color::Yellow)
            } else if total_pkts == 0 {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default().fg(Color::Green)
            };

            // Accumulate aggregates for the summary row
            agg_tx += rates.tx_rate;
            agg_rx += rates.rx_rate;
            agg_tx_err += rates.tx_err_rate;
            agg_rx_err += rates.rx_err_rate;

            let cells = vec![
                Cell::from(Span::raw(format!("P{}", port_num))),
                Cell::from(Span::styled(status_str, status_style)),
                Cell::from(Span::styled(
                    speed_str,
                    if speed_str == "--" {
                        Style::default().fg(Color::DarkGray)
                    } else {
                        Style::default().fg(Color::Green)
                    },
                )),
                Cell::from(Span::raw(tx_str)),
                Cell::from(Span::raw(rx_str)),
                Cell::from(Span::raw(tx_err_str)),
                Cell::from(Span::raw(rx_err_str)),
                Cell::from(Span::styled(err_str, err_style)),
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(8),
        Constraint::Length(6),
        Constraint::Length(8),
        Constraint::Length(8),
        Constraint::Length(7),
        Constraint::Length(7),
        Constraint::Length(7),
    ];

    // Build aggregate totals row
    let agg_total_pkts = agg_tx + agg_rx + agg_tx_err + agg_rx_err;
    let agg_total_errs = agg_tx_err + agg_rx_err;
    let agg_err_pct = if agg_total_pkts > 0 {
        (agg_total_errs as f64 / agg_total_pkts as f64) * 100.0
    } else {
        0.0
    };
    let agg_err_str = if agg_err_pct < 0.01 && agg_total_pkts > 0 {
        "<0.01%".to_string()
    } else {
        format!("{:.2}%", agg_err_pct)
    };

    let agg_row = Row::new(vec![
        Cell::from(Span::styled(
            "Σ",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::raw("")),
        Cell::from(Span::raw("")),
        Cell::from(Span::styled(
            format_rate(agg_tx),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            format_rate(agg_rx),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            format_rate(agg_tx_err),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            format_rate(agg_rx_err),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Cell::from(Span::styled(
            agg_err_str,
            Style::default().add_modifier(Modifier::BOLD),
        )),
    ])
    .style(Style::default().bg(Color::DarkGray));

    let mut all_rows = rows;
    all_rows.push(agg_row);

    let table = Table::new(all_rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " Ports ({} up) ",
                port_data
                    .iter()
                    .filter(|(_, p)| p.link_status != "Link Down" && p.port_status == "Enabled")
                    .count()
            ))
            .border_style(border_style),
    );

    f.render_widget(table, table_area);

    // --- Sparkline section ---
    if let Some(spark_area) = spark_area {
        render_sparklines(f, app, spark_area);
    }
}

// ---------------------------------------------------------------------------
// Sparkline section (rendered below the port table)
// ---------------------------------------------------------------------------

fn render_sparklines(f: &mut Frame, app: &TuiApp, area: Rect) {
    let current = app.current();
    let stats = match &current.port_stats {
        Some(s) => s,
        None => return,
    };

    let port_data: Vec<(usize, &PortStats)> = stats.ports().into_iter().enumerate().collect();
    let active: Vec<(usize, &PortStats)> = port_data
        .into_iter()
        .filter(|(_, p)| p.link_status != "Link Down" && p.port_status == "Enabled")
        .collect();

    if active.is_empty() {
        return;
    }

    // Layout: one row per active port
    let constraints: Vec<Constraint> = (0..active.len()).map(|_| Constraint::Length(1)).collect();

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let max_val = current
        .port_histories
        .iter()
        .flat_map(|h| h.ring.iter())
        .max()
        .copied()
        .unwrap_or(1)
        .max(1);

    for (idx, (port_idx, _p)) in active.iter().enumerate() {
        if idx >= rows.len() {
            break;
        }

        let history = &current.port_histories[*port_idx];
        let data: Vec<u64> = history.ring.iter().copied().collect();

        if data.is_empty() {
            continue;
        }

        let port_num = port_idx + 1;
        let rates = &current.port_rates[*port_idx];
        let label = format!(
            "P{} {:>6}/{:>6}",
            port_num,
            format_rate(rates.tx_rate),
            format_rate(rates.rx_rate)
        );

        let sparkline = Sparkline::default()
            .data(&data)
            .max(max_val)
            .style(Style::default().fg(Color::Cyan));

        // Render label + sparkline on the same row
        let label_span = Span::styled(label, Style::default().fg(Color::DarkGray));
        let label_para = Paragraph::new(Line::from(label_span));
        f.render_widget(label_para, rows[idx]);

        // Offset sparkline to the right of the label
        let spark_rect = Rect {
            x: rows[idx].x + 22,
            y: rows[idx].y,
            width: rows[idx].width.saturating_sub(22),
            height: 1,
        };
        f.render_widget(sparkline, spark_rect);
    }
}

// ---------------------------------------------------------------------------
// MAC pane
// ---------------------------------------------------------------------------

fn render_mac_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let border_style = if app.active_pane == 1 {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let current = app.current();
    let entries = match &current.mac_entries {
        Some(e) => e,
        None => {
            let p = Paragraph::new("Waiting for data...").block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" MAC Table ")
                    .border_style(border_style),
            );
            f.render_widget(p, area);
            return;
        }
    };

    let max_visible = (area.height as usize).saturating_sub(3);
    let scroll = app
        .scroll_offset_mac
        .min(entries.len().saturating_sub(max_visible));

    let header_cells = ["MAC Address", "VLAN", "Port", "Age"].iter().map(|h| {
        Cell::from(Span::styled(
            *h,
            Style::default().add_modifier(Modifier::BOLD),
        ))
    });
    let header =
        Row::new(header_cells).style(Style::default().bg(Color::DarkGray).fg(Color::White));

    let rows: Vec<Row> = entries
        .iter()
        .skip(scroll)
        .take(max_visible)
        .map(|e| {
            Row::new(vec![
                Cell::from(Span::raw(e.mac_addr.clone())),
                Cell::from(Span::raw(e.vlan_id.to_string())),
                Cell::from(Span::raw(e.port_id.to_string())),
                Cell::from(Span::raw(format!("{}s", e.age_timer))),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(20),
        Constraint::Length(6),
        Constraint::Length(6),
        Constraint::Length(8),
    ];

    let table = Table::new(rows, widths).header(header).block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(
                " MAC Table ({})  churn: {} ",
                entries.len(),
                current.mac_churn
            ))
            .border_style(border_style),
    );

    f.render_widget(table, area);
}

// ---------------------------------------------------------------------------
// Config / Edit pane — full interactive settings editor
// ---------------------------------------------------------------------------

/// Represents one row in the config editor: either a section header or a field.
struct ConfigField {
    label: String,
    value: String,
    is_header: bool,
    section_idx: usize,
    field_idx: usize,
}

/// Build the complete list of config fields for the current switch.
///
/// NOTE: This function makes multiple HTTP requests to the switch on every
/// render call (get_network_settings, get_port_vlan, get_stp_config,
/// get_igmp_config, get_storm_control, get_port_mirror, get_trunk_config).
/// On real switches, this can cause lag at low refresh intervals.
/// FIXME: Cache these in SwitchData and refresh them in SwitchData::refresh()
/// alongside the already-cached system_info, port_settings, and mac_entries.
fn config_fields(app: &TuiApp) -> Vec<ConfigField> {
    let current = app.current();
    let mut fields = Vec::new();
    let mut s = 0;

    // --- System ---
    let info = current.system_info.as_ref();
    fields.push(ConfigField {
        label: "── System ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    if let Some(i) = info {
        fields.push(ConfigField {
            label: "Description".into(),
            value: i.des.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 1,
        });
    }
    s += 1;

    // --- Network ---
    let net = &current.network_settings;
    fields.push(ConfigField {
        label: "── Network ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    if let Some(n) = &net {
        let dhcp = if n.dhcp_enabled == "1" {
            "DHCP"
        } else {
            "Static"
        };
        fields.push(ConfigField {
            label: "  IP Address".into(),
            value: n.ip_address.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 1,
        });
        fields.push(ConfigField {
            label: "  Netmask".into(),
            value: n.netmask.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 2,
        });
        fields.push(ConfigField {
            label: "  Gateway".into(),
            value: n.gateway.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 3,
        });
        fields.push(ConfigField {
            label: "  DNS".into(),
            value: n.dns_server.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 4,
        });
        fields.push(ConfigField {
            label: "  Mode".into(),
            value: dhcp.to_string(),
            is_header: false,
            section_idx: s,
            field_idx: 5,
        });
    }
    s += 1;

    // --- Port Settings (per port) ---
    let ports = current.port_settings.as_ref();
    for port_id in 1..=10u32 {
        fields.push(ConfigField {
            label: format!("── Port {} ──", port_id),
            value: String::new(),
            is_header: true,
            section_idx: s,
            field_idx: 0,
        });
        if let Some(ps) = ports {
            let port_list = ps.ports();
            let p = port_list.get((port_id - 1) as usize);
            if let Some(p) = p {
                fields.push(ConfigField {
                    label: "  Status".into(),
                    value: p.port_status.clone(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 1,
                });
                fields.push(ConfigField {
                    label: "  Speed".into(),
                    value: p.spd_duplex_cfg.clone(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 2,
                });
                fields.push(ConfigField {
                    label: "  Flow Ctrl".into(),
                    value: p.flow_ctrl_cfg.clone(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 3,
                });
            }
        }
        s += 1;
    }

    // --- VLAN (per port) ---
    let vlan = &current.port_vlan;
    for port_id in 1..=10u32 {
        fields.push(ConfigField {
            label: format!("── VLAN Port {} ──", port_id),
            value: String::new(),
            is_header: true,
            section_idx: s,
            field_idx: 0,
        });
        if let Some(v) = &vlan {
            let vlan_list = v.ports();
            let e = vlan_list.get((port_id - 1) as usize);
            if let Some(e) = e {
                fields.push(ConfigField {
                    label: "  PVID".into(),
                    value: e.pvid.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 1,
                });
                let ft = match e.frame_type {
                    0 => "All",
                    1 => "Tagged",
                    2 => "Untagged",
                    _ => "?",
                };
                fields.push(ConfigField {
                    label: "  Frame Type".into(),
                    value: ft.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 2,
                });
            }
        }
        s += 1;
    }

    // --- STP ---
    let stp = &current.stp_config;
    fields.push(ConfigField {
        label: "── STP ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    if let Some(st) = &stp {
        let state = if st.stp_enable == "1" {
            "Enabled"
        } else {
            "Disabled"
        };
        fields.push(ConfigField {
            label: "  State".into(),
            value: state.to_string(),
            is_header: false,
            section_idx: s,
            field_idx: 1,
        });
    }
    s += 1;

    // --- IGMP ---
    let igmp = &current.igmp_config;
    fields.push(ConfigField {
        label: "── IGMP ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    if let Some(ig) = &igmp {
        let onoff = |v: &str| if v == "on" { "On" } else { "Off" };
        fields.push(ConfigField {
            label: "  Snooping".into(),
            value: onoff(&ig.igmp).to_string(),
            is_header: false,
            section_idx: s,
            field_idx: 1,
        });
        fields.push(ConfigField {
            label: "  Fast Leave".into(),
            value: onoff(&ig.fast_leave).to_string(),
            is_header: false,
            section_idx: s,
            field_idx: 2,
        });
        fields.push(ConfigField {
            label: "  Report Flood".into(),
            value: onoff(&ig.report_flood).to_string(),
            is_header: false,
            section_idx: s,
            field_idx: 3,
        });
    }
    s += 1;

    // --- Storm Control (per port) ---
    let storm = &current.storm_control;
    for port_id in 1..=10u32 {
        fields.push(ConfigField {
            label: format!("── Storm Port {} ──", port_id),
            value: String::new(),
            is_header: true,
            section_idx: s,
            field_idx: 0,
        });
        if let Some(sc) = &storm {
            if let Some(p) = sc.ports.iter().find(|p| p.port_id == port_id) {
                fields.push(ConfigField {
                    label: "  Broadcast".into(),
                    value: p.sctrl_bcast.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 1,
                });
                fields.push(ConfigField {
                    label: "  Multicast".into(),
                    value: p.sctrl_mcast.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 2,
                });
                fields.push(ConfigField {
                    label: "  Unicast".into(),
                    value: p.sctrl_unucast.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 3,
                });
                fields.push(ConfigField {
                    label: "  UnMcast".into(),
                    value: p.sctrl_unmcast.to_string(),
                    is_header: false,
                    section_idx: s,
                    field_idx: 4,
                });
            }
        }
        s += 1;
    }

    // --- Mirror ---
    let mirror = &current.port_mirror;
    fields.push(ConfigField {
        label: "── Mirror ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    if let Some(m) = &mirror {
        fields.push(ConfigField {
            label: "  Monitor Port".into(),
            value: m.monitoring_port_id.clone(),
            is_header: false,
            section_idx: s,
            field_idx: 1,
        });
    }
    s += 1;

    // --- Trunk (per port) ---
    let trunk = &current.trunk_config;
    for port_id in 1..=10u32 {
        fields.push(ConfigField {
            label: format!("── Trunk Port {} ──", port_id),
            value: String::new(),
            is_header: true,
            section_idx: s,
            field_idx: 0,
        });
        if let Some(t) = &trunk {
            let raw = &t.raw;
            let port_key = format!("Port_{}", port_id);
            let type_val = raw
                .get(&port_key)
                .and_then(|v| v.get(format!("portTypeId_{}", port_id)))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let grp_val = raw
                .get(format!("Port_{}_grpInd", port_id))
                .and_then(|v| v.as_u64())
                .unwrap_or(0);
            let type_str = match type_val {
                0 => "Static",
                1 => "LACP",
                2 => "None",
                _ => "?",
            };
            fields.push(ConfigField {
                label: "  Type".into(),
                value: type_str.to_string(),
                is_header: false,
                section_idx: s,
                field_idx: 1,
            });
            fields.push(ConfigField {
                label: "  Group".into(),
                value: grp_val.to_string(),
                is_header: false,
                section_idx: s,
                field_idx: 2,
            });
        }
        s += 1;
    }

    // --- Loop Protection ---
    // NOTE: Shows hardcoded defaults rather than fetching from the switch.
    // The GET /port_lock_cfg.json endpoint returns actual values but is not
    // called here. FIXME: Add get_loop_protection_config() and fetch from switch.
    fields.push(ConfigField {
        label: "── Loop Protection ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    fields.push(ConfigField {
        label: "  Enabled".into(),
        value: "Off".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 1,
    });
    fields.push(ConfigField {
        label: "  Interval (s)".into(),
        value: "10".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 2,
    });
    fields.push(ConfigField {
        label: "  Recover (s)".into(),
        value: "2".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 3,
    });
    s += 1;

    // --- Management ---
    fields.push(ConfigField {
        label: "── Management ──".into(),
        value: String::new(),
        is_header: true,
        section_idx: s,
        field_idx: 0,
    });
    fields.push(ConfigField {
        label: "  Clear Statistics".into(),
        value: "[Enter to clear]".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 1,
    });
    fields.push(ConfigField {
        label: "  Clear MAC Table".into(),
        value: "[Enter to clear]".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 2,
    });
    fields.push(ConfigField {
        label: "  Save Config".into(),
        value: "[Enter to save]".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 3,
    });
    fields.push(ConfigField {
        label: "  Factory Reset".into(),
        value: "[type 'yes' to confirm]".to_string(),
        is_header: false,
        section_idx: s,
        field_idx: 4,
    });

    fields
}

fn config_field_count(app: &TuiApp) -> usize {
    config_fields(app).len()
}

fn config_field_at(app: &TuiApp, idx: usize) -> ConfigField {
    let fields = config_fields(app);
    fields.into_iter().nth(idx).unwrap_or(ConfigField {
        label: "─".into(),
        value: String::new(),
        is_header: true,
        section_idx: 0,
        field_idx: 0,
    })
}

fn render_config_pane(f: &mut Frame, app: &TuiApp, area: Rect) {
    let current = app.current();
    let border_style = Style::default().fg(Color::Cyan);

    let mock_label = if current.client.is_mock() {
        " [MOCK MODE — safe] "
    } else {
        " [LIVE SWITCH] "
    };

    let fields = config_fields(app);
    let max_visible = (area.height as usize).saturating_sub(3);
    let visible_start = app
        .config_cursor
        .saturating_sub(2)
        .min(fields.len().saturating_sub(max_visible));
    let _visible_end = (visible_start + max_visible).min(fields.len());

    let mut lines: Vec<Line> = Vec::new();
    lines.push(Line::from(vec![Span::styled(
        mock_label,
        Style::default().fg(Color::Red).bg(Color::White),
    )]));
    lines.push(Line::from(vec![Span::styled(
        "Keys: ↑↓=navigate  Enter=edit  a=apply  s=save  Tab=next pane  ←→=switch  q=quit",
        Style::default().fg(Color::DarkGray),
    )]));

    if app.editing {
        lines.push(Line::from(vec![
            Span::styled(" EDITING: ", Style::default().fg(Color::Yellow)),
            Span::styled(
                &app.edit_buffer,
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "  [Enter=commit Esc=cancel]",
                Style::default().fg(Color::DarkGray),
            ),
        ]));
    } else {
        lines.push(Line::from(""));
    }

    lines.push(Line::from(""));

    for (i, field) in fields
        .iter()
        .enumerate()
        .skip(visible_start)
        .take(max_visible)
    {
        let cursor = if i == app.config_cursor && !app.editing {
            "▶ "
        } else {
            "  "
        };
        let cursor_style = if i == app.config_cursor {
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        if field.is_header {
            lines.push(Line::from(vec![Span::styled(
                format!("{}{}", cursor, field.label),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )]));
        } else {
            let val_display = if i == app.config_cursor && app.editing {
                app.edit_buffer.to_string()
            } else {
                field.value.clone()
            };
            lines.push(Line::from(vec![
                Span::styled(format!("{}{:<20}", cursor, field.label), cursor_style),
                Span::raw(" │ "),
                Span::raw(val_display),
            ]));
        }
    }

    let p = Paragraph::new(lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Config ")
            .border_style(border_style),
    );
    f.render_widget(p, area);
}

/// Apply the value in `edit_buffer` to the field at `config_cursor`.
fn apply_config_field(app: &mut TuiApp) -> Result<()> {
    let field = config_field_at(app, app.config_cursor);
    let val = app.edit_buffer.trim().to_string();
    if val.is_empty() {
        return Ok(());
    }

    let client = &app.switches[app.current_switch].client;
    let s = field.section_idx;

    // Determine which field to update based on section_idx pattern.
    // Sections: 0=System, 1=Network, 2-11=Ports, 12-21=VLAN, 22=STP, 23=IGMP, 24-33=Storm
    if s == 0 {
        // System
        client.set_description(&val)?;
    } else if s == 1 {
        // Network
        let mut req = NetworkSettingsRequest::default();
        match field.field_idx {
            1 => req.ip_address = Some(val),
            2 => req.netmask = Some(val),
            3 => req.gateway = Some(val),
            4 => req.dns_server = Some(val),
            5 => {
                req.dhcp_enabled = Some(if val.to_lowercase().contains("dhcp") {
                    "1".to_string()
                } else {
                    "0".to_string()
                })
            }
            _ => {}
        }
        let has_change = req.ip_address.is_some()
            || req.netmask.is_some()
            || req.gateway.is_some()
            || req.dns_server.is_some()
            || req.dhcp_enabled.is_some();
        if has_change {
            client.set_network_settings(&req)?;
        }
    } else if (2..=11).contains(&s) {
        // Port settings
        let port_id = (s - 1) as u32; // sections 2-11 → ports 1-10
        let ps = client.get_port_settings()?;
        let port_cfg = ps
            .ports()
            .get((port_id - 1) as usize)
            .map(|&p| p.clone())
            .unwrap_or(ps.port_1.clone());
        let (status, speed, flow) = match field.field_idx {
            1 => (
                Some(val),
                Some(port_cfg.spd_duplex_cfg.clone()),
                Some(port_cfg.flow_ctrl_cfg.clone()),
            ),
            2 => (
                Some(port_cfg.port_status.clone()),
                Some(val),
                Some(port_cfg.flow_ctrl_cfg.clone()),
            ),
            3 => (
                Some(port_cfg.port_status.clone()),
                Some(port_cfg.spd_duplex_cfg.clone()),
                Some(val),
            ),
            _ => return Ok(()),
        };
        let req = PortSettingsRequest {
            port_status: status,
            spd_duplex_cfg: speed,
            flow_ctrl_cfg: flow,
        };
        client.set_port_settings(&PortSettingsApplyRequest::single_port(port_id, req)?)?;
    } else if (12..=21).contains(&s) {
        // VLAN
        let port_id = (s - 11) as u32;
        let vlan_resp = client.get_port_vlan()?;
        let ve = vlan_resp.ports().get((port_id - 1) as usize).cloned();
        let (pvid, ft) = match ve {
            Some(e) => (
                if field.field_idx == 1 {
                    val.parse().unwrap_or(e.pvid)
                } else {
                    e.pvid
                },
                if field.field_idx == 2 {
                    match val.as_str() {
                        "All" => 0,
                        "Tagged" => 1,
                        "Untagged" => 2,
                        _ => e.frame_type,
                    }
                } else {
                    e.frame_type
                },
            ),
            None => (1, 0),
        };
        client.set_port_vlan(&PortVlanRequest::single_port(port_id, pvid, ft)?)?;
    } else if s == 22 {
        // STP
        let en = val.to_lowercase().contains("enabled");
        let req = if en {
            StpConfigRequest::enable()
        } else {
            StpConfigRequest::disable()
        };
        client.set_stp_config(&req)?;
    } else if s == 23 {
        // IGMP
        let ig = client.get_igmp_config()?;
        let onoff = |b: bool| if b { "on" } else { "off" };
        let new_val = val.to_lowercase().contains("on");
        let r = IgmpConfigRequest {
            igmp: Some(if field.field_idx == 1 {
                onoff(new_val)
            } else {
                &ig.igmp
            })
            .map(|s| s.to_string()),
            fast_leave: Some(if field.field_idx == 2 {
                onoff(new_val)
            } else {
                &ig.fast_leave
            })
            .map(|s| s.to_string()),
            report_flood: Some(if field.field_idx == 3 {
                onoff(new_val)
            } else {
                &ig.report_flood
            })
            .map(|s| s.to_string()),
        };
        client.set_igmp_config(&r)?;
    } else if (24..=33).contains(&s) {
        // Storm control
        let port_id = (s - 23) as u32;
        let sc = client.get_storm_control()?;
        let p = sc
            .ports
            .iter()
            .find(|p| p.port_id == port_id)
            .cloned()
            .unwrap_or(StormControlPort {
                port_id,
                sctrl_bcast: 0,
                sctrl_mcast: 0,
                sctrl_unucast: 0,
                sctrl_unmcast: 0,
            });
        let v: u32 = val.parse().unwrap_or(0);
        let req = StormControlRequest {
            portnum: 10,
            ports: vec![StormControlPortRequest {
                port_id,
                sctrl_bcast: if field.field_idx == 1 {
                    v
                } else {
                    p.sctrl_bcast
                },
                sctrl_mcast: if field.field_idx == 2 {
                    v
                } else {
                    p.sctrl_mcast
                },
                sctrl_unucast: if field.field_idx == 3 {
                    v
                } else {
                    p.sctrl_unucast
                },
                sctrl_unmcast: if field.field_idx == 4 {
                    v
                } else {
                    p.sctrl_unmcast
                },
            }],
        };
        client.set_storm_control(&req)?;
    } else if s == 34 {
        // Mirror
        // NOTE: Only monitor_port_id is editable. Source port ingress/egress
        // configuration is not exposed in the TUI. Setting the monitor port
        // sends a PortMirrorRequest with all source ports as None, which may
        // clear existing source port mirror config on real hardware.
        // FIXME: Expose source_ingress/source_egress fields, or preserve
        // existing entries by fetching and merging.
        let v: u32 = val.parse().unwrap_or(0);
        let req = PortMirrorRequest {
            port_num: "10".to_string(),
            monitoring_port_id: v.to_string(),
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
        client.set_port_mirror(&req)?;
    } else if (35..=44).contains(&s) {
        // Trunk (per port)
        let port_id = (s - 34) as u32;
        let val_lower = val.to_lowercase();
        let trunk_type: u32 = if val_lower.contains("lacp") {
            1
        } else if val_lower.contains("static") {
            0
        } else {
            2
        };
        let group: u32 = val.parse().unwrap_or(0);
        let entry = TrunkPortEntry {
            port_type: if field.field_idx == 1 {
                Some(trunk_type)
            } else {
                None
            },
            port_priority: Some(128),
            lacp_timeout: Some(0),
            group_index: if field.field_idx == 2 {
                Some(group)
            } else {
                None
            },
            state: Some(if trunk_type == 2 { 0 } else { 1 }),
        };
        let mut req = TrunkConfigRequest {
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
        match port_id {
            1 => req.port_1 = Some(entry),
            2 => req.port_2 = Some(entry),
            3 => req.port_3 = Some(entry),
            4 => req.port_4 = Some(entry),
            5 => req.port_5 = Some(entry),
            6 => req.port_6 = Some(entry),
            7 => req.port_7 = Some(entry),
            8 => req.port_8 = Some(entry),
            9 => req.port_9 = Some(entry),
            10 => req.port_10 = Some(entry),
            _ => {}
        }
        client.set_trunk_config(&req)?;
    } else if s == 45 {
        // Loop Protection
        let req = LoopProtectionRequest {
            port_num: Some("10".to_string()),
            detect_enable: if field.field_idx == 1 {
                Some(if val.to_lowercase().contains("on") || val == "1" {
                    "1".to_string()
                } else {
                    "0".to_string()
                })
            } else {
                None
            },
            time_interval: if field.field_idx == 2 {
                Some(val.clone())
            } else {
                None
            },
            recover_time: if field.field_idx == 3 {
                Some(val)
            } else {
                None
            },
        };
        client.set_loop_protection(&req)?;
    } else if s == 46 {
        // Management
        match field.field_idx {
            1 => {
                client.clear_statistics()?;
            }
            2 => {
                client.clear_mac_entries()?;
            }
            3 => {
                client.save_config()?;
            }
            4 if val.to_lowercase().trim() == "yes" => {
                client.factory_reset()?;
            }
            _ => {}
        }
    }
    Ok(())
}

/// Apply current description + save (catches up any left-over changes).
fn apply_all_config(app: &mut TuiApp) -> Result<()> {
    let client = &app.switches[app.current_switch].client;
    let info = client.get_system_info()?;
    client.set_description(&info.des)?;
    client.save_config()?;
    Ok(())
}

fn render_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let pane_name = match app.active_pane {
        0 => "Ports",
        1 => "MAC",
        2 => "Config",
        _ => "",
    };

    let current = app.current();
    let switch_label = if current.target.name != current.target.host {
        format!("Switch: {} ({})", current.target.name, current.target.host)
    } else {
        format!("Switch: {}", current.target.host)
    };

    let time_str = match &current.last_refresh {
        Some(t) => t.format("%H:%M:%S").to_string(),
        None => "never".to_string(),
    };

    let mut parts = vec![
        Span::styled(" [q] Quit ", Style::default().fg(Color::Yellow)),
        Span::styled(" [r] Refresh ", Style::default().fg(Color::Yellow)),
        Span::styled(
            format!(" [+/-] {}s ", app.refresh_interval),
            Style::default().fg(Color::Yellow),
        ),
        Span::styled(
            format!(" [←→] {} ", switch_label),
            Style::default().fg(Color::Cyan),
        ),
        Span::raw(format!(" | Pane: {}", pane_name)),
        Span::raw(format!(" | Last: {}", time_str)),
    ];

    if let Some(err) = &current.error {
        parts.push(Span::styled(
            format!(" | ERROR: {}", err),
            Style::default().fg(Color::Red),
        ));
    }

    let p =
        Paragraph::new(Line::from(parts)).style(Style::default().bg(Color::Black).fg(Color::White));
    f.render_widget(p, area);
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn shorten_speed(s: &str) -> &str {
    if s.contains("2500Mbps") {
        "2.5G"
    } else if s.contains("1000Mbps") {
        "1G"
    } else if s.contains("100Mbps") {
        "100M"
    } else if s.contains("10G") {
        "10G"
    } else {
        s
    }
}

fn format_rate(rate: u64) -> String {
    if rate >= 1_000_000 {
        format!("{:.1}M", rate as f64 / 1_000_000.0)
    } else if rate >= 1_000 {
        format!("{:.1}K", rate as f64 / 1_000.0)
    } else {
        rate.to_string()
    }
}
