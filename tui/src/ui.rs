use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table, Wrap},
    Frame,
};
use taivas_types::{AspectType, ZodiacSign};

use crate::app::App;
use crate::astro::{compass, format_utc, TwilightStatus};
use crate::guidance::GuidanceStatus;

// ── Entry point ───────────────────────────────────────────────────────────────

pub fn render(f: &mut Frame, app: &mut App) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(2),
        ])
        .split(f.area());

    render_header(f, app, layout[0]);

    match app.active_tab {
        0 => render_space_weather(f, app, layout[1]),
        1 => render_night_sky(f, app, layout[1]),
        2 => render_planets(f, app, layout[1]),
        3 => render_reading(f, app, layout[1]),
        _ => {}
    }

    render_footer(f, app, layout[2]);
}

// ── Header / tab bar ─────────────────────────────────────────────────────────

fn render_header(f: &mut Frame, app: &App, area: Rect) {
    let tabs = ["  Space Weather  ", "  Sky Now  ", "  Planets  ", "  Reading  "];

    let mut spans = vec![Span::styled(
        " ✦ CosmicForces ",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];

    for (i, &name) in tabs.iter().enumerate() {
        spans.push(Span::styled(
            "│",
            Style::default().fg(Color::DarkGray),
        ));
        if i == app.active_tab {
            spans.push(Span::styled(
                name,
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                name,
                Style::default().fg(Color::DarkGray),
            ));
        }
    }

    let p = Paragraph::new(Line::from(spans))
        .block(Block::default().borders(Borders::ALL).border_style(
            Style::default().fg(Color::DarkGray),
        ));
    f.render_widget(p, area);
}

// ── Footer ────────────────────────────────────────────────────────────────────

fn render_footer(f: &mut Frame, app: &App, area: Rect) {
    let line = Line::from(vec![
        Span::styled(" [q]", Style::default().fg(Color::Red)),
        Span::raw(" Quit  "),
        Span::styled("[Tab ←→]", Style::default().fg(Color::Cyan)),
        Span::raw(" Switch  "),
        Span::styled("[r]", Style::default().fg(Color::Green)),
        Span::raw(" Refresh  "),
        Span::styled("[↑↓ jk]", Style::default().fg(Color::Yellow)),
        Span::raw(" Scroll      "),
        Span::styled(
            &app.status,
            Style::default().fg(Color::DarkGray),
        ),
    ]);
    f.render_widget(
        Paragraph::new(line)
            .block(Block::default().borders(Borders::TOP).border_style(
                Style::default().fg(Color::DarkGray),
            )),
        area,
    );
}

// ── Tab 1: Space Weather ─────────────────────────────────────────────────────

fn intensity_color(intensity: &str) -> Color {
    match intensity.chars().next() {
        Some('X') => Color::Red,
        Some('M') => Color::Yellow,
        Some('C') => Color::Green,
        _ => Color::White,
    }
}

fn class_rank(intensity: &str) -> i32 {
    let class = intensity.chars().next().unwrap_or('B');
    let num: f64 = intensity.get(1..).unwrap_or("0").parse().unwrap_or(0.0);
    match class {
        'X' => 4_000 + (num * 10.0) as i32,
        'M' => 3_000 + (num * 10.0) as i32,
        'C' => 2_000 + (num * 10.0) as i32,
        'B' => 1_000 + (num * 10.0) as i32,
        _ => 0,
    }
}

fn render_space_weather(f: &mut Frame, app: &mut App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Summary bar
    let strongest = app
        .solar_events
        .iter()
        .max_by_key(|e| class_rank(&e.intensity))
        .map(|e| e.intensity.as_str())
        .unwrap_or("None");

    let (aurora_label, aurora_color) = match strongest.chars().next() {
        Some('X') => ("High", Color::Red),
        Some('M') => ("Moderate", Color::Yellow),
        Some('C') => ("Low", Color::Green),
        _ => ("None", Color::DarkGray),
    };

    let x_count = app.solar_events.iter().filter(|e| e.intensity.starts_with('X')).count();
    let m_count = app.solar_events.iter().filter(|e| e.intensity.starts_with('M')).count();

    let summary = Paragraph::new(Line::from(vec![
        Span::raw("  Flares (30d): "),
        Span::styled(
            format!("{}", app.solar_events.len()),
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!("  (X:{x_count}  M:{m_count})   Strongest: ")),
        Span::styled(
            strongest,
            Style::default()
                .fg(intensity_color(strongest))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   Aurora potential: "),
        Span::styled(
            aurora_label,
            Style::default().fg(aurora_color).add_modifier(Modifier::BOLD),
        ),
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Activity Summary ")
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(summary, chunks[0]);

    // Events table
    let header = Row::new(["#", "Type", "Peak Time (UTC)", "Class", "Active Region"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = app
        .solar_events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let color = intensity_color(&e.intensity);
            Row::new([
                Cell::from(format!("{}", i + 1)),
                Cell::from(e.event_type.clone()),
                Cell::from(e.peak_time.clone()),
                Cell::from(e.intensity.clone())
                    .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
                Cell::from(e.active_region.clone().unwrap_or_else(|| "—".to_string())),
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(4),
        Constraint::Length(8),
        Constraint::Length(24),
        Constraint::Length(8),
        Constraint::Min(12),
    ];

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Solar Events — Last 30 Days (most recent first) ")
                .border_style(Style::default().fg(Color::DarkGray)),
        )
        .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED))
        .highlight_symbol("▶ ");

    f.render_stateful_widget(table, chunks[1], &mut app.events_table);
}

// ── Tab 2: Sky Now ────────────────────────────────────────────────────────────

fn twilight_color(t: TwilightStatus) -> Color {
    match t {
        TwilightStatus::Day => Color::White,
        TwilightStatus::Civil => Color::Yellow,
        TwilightStatus::Nautical => Color::LightBlue,
        TwilightStatus::Astronomical => Color::Cyan,
        TwilightStatus::Night => Color::Blue,
    }
}

fn render_night_sky(f: &mut Frame, app: &App, area: Rect) {
    let sky = &app.sky;

    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(7), Constraint::Min(0)])
        .split(area);

    let top = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[0]);

    let bot = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(rows[1]);

    // Observer block
    let obs_text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Location  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                &app.observer.name,
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Lat/Lon   ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:.2}°{}  {:.2}°{}",
                app.observer.lat.abs(),
                if app.observer.lat >= 0.0 { "N" } else { "S" },
                app.observer.lon.abs(),
                if app.observer.lon >= 0.0 { "E" } else { "W" }
            )),
        ]),
        Line::from(vec![
            Span::styled("  UTC       ", Style::default().fg(Color::DarkGray)),
            Span::raw(chrono::Utc::now().format("%H:%M:%S").to_string()),
        ]),
        Line::raw(""),
    ];
    f.render_widget(
        Paragraph::new(obs_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Observer ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        top[0],
    );

    // Conditions block
    let tc = twilight_color(sky.twilight);
    let qc = match sky.twilight {
        TwilightStatus::Day => Color::Red,
        TwilightStatus::Civil | TwilightStatus::Nautical => Color::Yellow,
        TwilightStatus::Astronomical | TwilightStatus::Night => Color::Green,
    };
    let cond_text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Status    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                sky.twilight.label(),
                Style::default().fg(tc).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Sun       ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:+.1}° alt  {}° az ({})  {} {}",
                sky.sun_alt,
                sky.sun_az as u32,
                compass(sky.sun_az),
                ZodiacSign::from_longitude(sky.sun_longitude).glyph(),
                ZodiacSign::from_longitude(sky.sun_longitude),
            )),
        ]),
        Line::from(vec![
            Span::styled("  Observing ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                sky.twilight.obs_stars(),
                Style::default().fg(Color::Yellow),
            ),
            Span::raw("  "),
            Span::styled(
                sky.twilight.obs_quality(),
                Style::default().fg(qc),
            ),
        ]),
        Line::raw(""),
    ];
    f.render_widget(
        Paragraph::new(cond_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Sky Conditions ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        top[1],
    );

    // Moon block
    let moon_sign = ZodiacSign::from_longitude(sky.moon_longitude);
    let moon_deg = ZodiacSign::degree_in_sign(sky.moon_longitude);
    let illum_bar = {
        let filled = (sky.moon_illumination / 100.0 * 20.0).round() as usize;
        let empty = 20usize.saturating_sub(filled);
        format!("[{}{}]", "█".repeat(filled), "░".repeat(empty))
    };
    let moon_color = if sky.moon_illumination > 90.0 {
        Color::Yellow
    } else if sky.moon_illumination > 50.0 {
        Color::White
    } else {
        Color::Gray
    };

    let moon_text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Phase       ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{}  ", sky.moon_phase_emoji)),
            Span::styled(
                sky.moon_phase_name,
                Style::default().fg(moon_color).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Illuminated ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:.0}%  {illum_bar}", sky.moon_illumination),
                Style::default().fg(moon_color),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Position    ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!(
                "{:.1}° {} {}  ({:.0}° az {})",
                moon_deg,
                moon_sign.glyph(),
                moon_sign,
                sky.moon_az,
                compass(sky.moon_az)
            )),
        ]),
        Line::from(vec![
            Span::styled("  Altitude    ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                format!("{:+.1}°", sky.moon_alt),
                Style::default().fg(if sky.moon_alt > 0.0 {
                    Color::Green
                } else {
                    Color::DarkGray
                }),
            ),
            Span::raw(if sky.moon_alt > 0.0 {
                "  above horizon"
            } else {
                "  below horizon"
            }),
        ]),
    ];
    f.render_widget(
        Paragraph::new(moon_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Moon ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        bot[0],
    );

    // Tonight's schedule block
    let fmt = |opt: Option<f64>| {
        opt.map(format_utc)
            .unwrap_or_else(|| "——:——".to_string())
    };

    let night_quality_note = match (sky.moon_illumination as u32, &sky.twilight) {
        (_, TwilightStatus::Day) => "  → Sun is up",
        (ill, _) if ill > 80 => "  → Bright moon limits DSO viewing",
        (ill, _) if ill > 40 => "  → Partial moon interference",
        _ => "  → Good conditions for deep sky",
    };

    let sched_text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Sunset       ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt(sky.sunset), Style::default().fg(Color::Yellow)),
            Span::raw(" UTC"),
        ]),
        Line::from(vec![
            Span::styled("  Astro dark   ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt(sky.astro_dusk), Style::default().fg(Color::Cyan)),
            Span::raw(" UTC"),
        ]),
        Line::from(vec![
            Span::styled("  Astro dawn   ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt(sky.astro_dawn), Style::default().fg(Color::Cyan)),
            Span::raw(" UTC"),
        ]),
        Line::from(vec![
            Span::styled("  Sunrise      ", Style::default().fg(Color::DarkGray)),
            Span::styled(fmt(sky.sunrise), Style::default().fg(Color::Yellow)),
            Span::raw(" UTC"),
        ]),
        Line::raw(""),
        Line::from(vec![Span::styled(
            night_quality_note,
            Style::default().fg(Color::DarkGray),
        )]),
    ];
    f.render_widget(
        Paragraph::new(sched_text).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Tonight's Window ")
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        bot[1],
    );
}

// ── Tab 3: Planets ────────────────────────────────────────────────────────────

fn aspect_color(at: &AspectType) -> Color {
    match at {
        AspectType::Conjunction => Color::White,
        AspectType::Sextile => Color::Green,
        AspectType::Square => Color::Red,
        AspectType::Trine => Color::Cyan,
        AspectType::Opposition => Color::Yellow,
    }
}

fn render_planets(f: &mut Frame, app: &App, area: Rect) {
    let Some(ref chart) = app.chart else {
        f.render_widget(
            Paragraph::new("Computing planetary positions...")
                .block(Block::default().borders(Borders::ALL)),
            area,
        );
        return;
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(4)])
        .split(area);

    // Planet table
    let header = Row::new(["", "Body", "Sign", "Longitude", "Altitude", "Azimuth", "Rx", "°/day"])
        .style(
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )
        .height(1);

    let rows: Vec<Row> = chart
        .planets
        .iter()
        .zip(
            app.sky
                .planet_altaz
                .iter()
                .chain(std::iter::repeat(&(0.0_f64, 0.0_f64))),
        )
        .map(|(planet, &(alt, az))| {
            let visible = alt > 0.0;
            let dim = Style::default().fg(Color::DarkGray);
            let normal = Style::default();

            let sign_str = format!("{} {}", planet.sign.glyph(), planet.sign);
            let lon_str = format!("{:.1}°", planet.longitude);
            let deg_str = format!("{:.1}°", planet.degree_in_sign);
            let alt_str = format!("{:+.1}°", alt);
            let az_str = format!("{:.0}° {}", az, compass(az));
            let rx_str = if planet.retrograde { "Rx" } else { "" };
            let spd_str = format!("{:+.3}", planet.speed);

            Row::new([
                Cell::from(planet.body.glyph()),
                Cell::from(planet.body.name()),
                Cell::from(sign_str),
                Cell::from(format!("{lon_str} ({deg_str})")),
                Cell::from(alt_str).style(if visible {
                    Style::default().fg(Color::Green)
                } else {
                    Style::default().fg(Color::DarkGray)
                }),
                Cell::from(az_str).style(if visible { normal } else { dim }),
                Cell::from(rx_str)
                    .style(Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)),
                Cell::from(spd_str).style(if planet.retrograde {
                    Style::default().fg(Color::Magenta)
                } else {
                    normal
                }),
            ])
            .style(if visible { normal } else { dim })
        })
        .collect();

    let widths = [
        Constraint::Length(3),   // glyph
        Constraint::Length(11),  // name
        Constraint::Length(14),  // sign
        Constraint::Length(16),  // longitude
        Constraint::Length(9),   // altitude
        Constraint::Length(10),  // azimuth
        Constraint::Length(4),   // Rx
        Constraint::Length(8),   // speed
    ];

    f.render_widget(
        Table::new(rows, widths)
            .header(header)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(
                        " Current Sky — {} ",
                        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC")
                    ))
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .row_highlight_style(Style::default().add_modifier(Modifier::REVERSED)),
        chunks[0],
    );

    // Aspects panel
    let aspect_count = chart.aspects.len();
    let aspect_title = format!(" Active Aspects ({aspect_count}) ");

    let content = if chart.aspects.is_empty() {
        Paragraph::new("  No major aspects currently active.")
            .style(Style::default().fg(Color::DarkGray))
    } else {
        let mut spans = vec![Span::raw("  ")];
        for (i, asp) in chart.aspects.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled("   ", Style::default()));
            }
            let applying = if asp.applying { "→" } else { "←" };
            spans.push(Span::styled(
                format!(
                    "{}{}{} {:.1}°{applying}",
                    asp.body1.glyph(),
                    asp.aspect_type.symbol(),
                    asp.body2.glyph(),
                    asp.orb
                ),
                Style::default().fg(aspect_color(&asp.aspect_type)),
            ));
        }
        Paragraph::new(Line::from(spans)).wrap(Wrap { trim: false })
    };

    f.render_widget(
        content.block(
            Block::default()
                .borders(Borders::ALL)
                .title(aspect_title)
                .border_style(Style::default().fg(Color::DarkGray)),
        ),
        chunks[1],
    );
}

// ── Tab 4: Reading ────────────────────────────────────────────────────────────

fn render_reading(f: &mut Frame, app: &App, area: Rect) {
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    let title = format!(" Your Reading — {today} ");

    let mut lines: Vec<Line> = Vec::new();

    match &app.daily_guidance.status {
        GuidanceStatus::Idle => {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "  Preparing your reading…",
                Style::default().fg(Color::DarkGray),
            )));
        }
        GuidanceStatus::Loading => {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "  \u{27f3} Consulting the ephemeris\u{2026}",
                Style::default().fg(Color::Cyan),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "  Ollama is generating your personalised astrological reading.",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  This takes 10\u{2013}30 seconds on the first run each day.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        GuidanceStatus::Error(e) => {
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "  \u{25b3} Could not reach Ollama",
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                format!("  {e}"),
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::raw(""));
            lines.push(Line::from(Span::styled(
                "  Make sure Ollama is running:  ollama serve",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  Then pull a model:  ollama pull mistral",
                Style::default().fg(Color::DarkGray),
            )));
            lines.push(Line::from(Span::styled(
                "  Press [r] to retry.",
                Style::default().fg(Color::DarkGray),
            )));
        }
        GuidanceStatus::Ready => {
            if let Some(ref cache) = app.daily_guidance.cache {
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(
                        cache.text.clone(),
                        Style::default().fg(Color::White),
                    ),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(Span::styled(
                    format!("  \u{2736} Reading for {}  \u{2022}  [j/k] to scroll", cache.date),
                    Style::default().fg(Color::DarkGray),
                )));
            }
        }
    }

    f.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(title)
                    .border_style(Style::default().fg(Color::DarkGray)),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.guidance_scroll, 0)),
        area,
    );
}
