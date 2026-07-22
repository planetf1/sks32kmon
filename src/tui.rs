use std::io;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use chrono::Local;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use ratatui::prelude::*;
use ratatui::widgets::*;
use ratatui::Terminal;

use crate::client::SwitchClient;
use crate::config::SwitchTarget;
use crate::models::*;

// ---------------------------------------------------------------------------
// Per-switch data snapshot
// ---------------------------------------------------------------------------

struct SwitchData {
    target: SwitchTarget,
    client: SwitchClient,
    system_info: Option<SystemInfo>,
    port_settings: Option<PortSettingsResponse>,
    port_stats: Option<PortStatisticsResponse>,
    prev_port_stats: Option<PortStatisticsResponse>,
    prev_stats_time: Option<Instant>,
    mac_entries: Option<Vec<MacEntry>>,
    last_refresh: Option<chrono::DateTime<Local>>,
    error: Option<String>,
}

impl SwitchData {
    fn new(target: SwitchTarget) -> Result<Self> {
        let client = SwitchClient::connect(&target.host, &target.user, &target.password)
            .with_context(|| format!("Failed to connect to {}", target.host))?;
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
                self.prev_port_stats = self.port_stats.take();
                self.prev_stats_time = if self.prev_port_stats.is_some() {
                    Some(now)
                } else {
                    None
                };
                self.port_stats = Some(stats);
            }
            Err(e) => {
                self.error = Some(format!("Port stats: {}", e));
                return;
            }
        }

        match self.client.get_dynamic_mac_entries() {
            Ok(entries) => self.mac_entries = Some(entries),
            Err(e) => {
                self.error = Some(format!("MAC: {}", e));
            }
        }

        self.last_refresh = Some(chrono::Local::now());
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
    active_pane: usize, // 0 = port pane, 1 = mac pane
    refresh_interval: u64, // seconds
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

pub fn run_tui(targets: &[SwitchTarget]) -> Result<()> {
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
        match SwitchData::new(target.clone()) {
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
    };

    app.refresh_all();

    loop {
        terminal.draw(|f| draw_ui(f, &app))?;

        // Poll for input at the current refresh interval
        if event::poll(Duration::from_secs(app.refresh_interval))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => break,
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            app.refresh_all();
                        }
                        KeyCode::Tab | KeyCode::Right => {
                            app.current_switch =
                                (app.current_switch + 1) % app.switches.len();
                            app.scroll_offset_port = 0;
                            app.scroll_offset_mac = 0;
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
                        KeyCode::Char('+') | KeyCode::Char('=') => {
                            app.refresh_interval = app.refresh_interval.saturating_sub(1).max(1);
                        }
                        KeyCode::Char('-') | KeyCode::Char('_') => {
                            app.refresh_interval = (app.refresh_interval + 1).min(60);
                        }
                        KeyCode::Up => match app.active_pane {
                            0 => {
                                if app.scroll_offset_port > 0 {
                                    app.scroll_offset_port -= 1;
                                }
                            }
                            1 => {
                                if app.scroll_offset_mac > 0 {
                                    app.scroll_offset_mac -= 1;
                                }
                            }
                            _ => {}
                        },
                        KeyCode::Down => match app.active_pane {
                            0 => app.scroll_offset_port += 1,
                            1 => app.scroll_offset_mac += 1,
                            _ => {}
                        },
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
    crossterm::execute!(terminal.backend_mut(), crossterm::terminal::LeaveAlternateScreen)?;
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
            let title = format!(
                " SKS3200-8E2X @ {}  |  FW: {}  HW: {}  MAC: {}  {}°C  |  {}",
                i.sys_ipv4, i.fw_ver, i.hw_ver, i.sys_macaddr, i.temperature, i.des
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
            tags.push(Span::styled(
                label,
                Style::default().fg(Color::DarkGray),
            ));
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
    let constraints = [Constraint::Percentage(55), Constraint::Percentage(45)];
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);

    render_port_pane(f, app, chunks[0]);
    render_mac_pane(f, app, chunks[1]);
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
            let p = Paragraph::new("Waiting for data...")
                .block(Block::default().borders(Borders::ALL).title(" Ports ").border_style(border_style));
            f.render_widget(p, area);
            return;
        }
    };

    let port_data: Vec<(usize, &PortStats)> = stats.ports().into_iter().enumerate().collect();
    let max_visible = (area.height as usize).saturating_sub(3);
    let scroll = app
        .scroll_offset_port
        .min(port_data.len().saturating_sub(max_visible));

    let header_cells = ["Port", "Status", "Speed", "Tx/s", "Rx/s"]
        .iter()
        .map(|h| Cell::from(Span::styled(*h, Style::default().add_modifier(Modifier::BOLD))));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

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

            // Packet rate computation
            let (tx_rate, rx_rate) =
                if let (Some(prev), Some(prev_time)) = (&current.prev_port_stats, &current.prev_stats_time)
                {
                    let elapsed = prev_time.elapsed().as_secs_f64().max(0.1);
                    let prev_ports = prev.ports();
                    let prev_p = prev_ports.get(*i);
                    match prev_p {
                        Some(pp) => {
                            let tx: f64 = p.tx_good_pkt.parse().unwrap_or(0.0);
                            let ptx: f64 = pp.tx_good_pkt.parse().unwrap_or(0.0);
                            let rx: f64 = p.rx_good_pkt.parse().unwrap_or(0.0);
                            let prx: f64 = pp.rx_good_pkt.parse().unwrap_or(0.0);
                            (
                                ((tx - ptx).max(0.0) / elapsed) as u64,
                                ((rx - prx).max(0.0) / elapsed) as u64,
                            )
                        }
                        None => (0, 0),
                    }
                } else {
                    (0, 0)
                };

            let tx_str = format_rate(tx_rate);
            let rx_str = format_rate(rx_rate);

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
            ];
            Row::new(cells)
        })
        .collect();

    let widths = [
        Constraint::Length(5),
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Length(10),
        Constraint::Length(10),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Ports ")
                .border_style(border_style),
        );

    f.render_widget(table, area);
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
            let p = Paragraph::new("Waiting for data...")
                .block(
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

    let header_cells = ["MAC Address", "VLAN", "Port", "Age"]
        .iter()
        .map(|h| Cell::from(Span::styled(*h, Style::default().add_modifier(Modifier::BOLD))));
    let header = Row::new(header_cells)
        .style(Style::default().bg(Color::DarkGray).fg(Color::White));

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

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(" MAC Table ({}) ", entries.len()))
                .border_style(border_style),
        );

    f.render_widget(table, area);
}

// ---------------------------------------------------------------------------
// Footer
// ---------------------------------------------------------------------------

fn render_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let pane_name = match app.active_pane {
        0 => "Ports",
        1 => "MAC",
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
            format!(" [Tab] {} ", switch_label),
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

    let p = Paragraph::new(Line::from(parts))
        .style(Style::default().bg(Color::Black).fg(Color::White));
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
