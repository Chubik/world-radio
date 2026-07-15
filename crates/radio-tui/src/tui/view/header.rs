use crate::tui::model::Model;
use crate::tui::theme::Palette;
use crate::tui::view::spectrum_render::render_grid;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub const SPECTRUM_H: u16 = 4;

/// height of the info block (system row + content row, plus optional song and
/// notice rows). the spectrum, when on, is drawn in SPECTRUM_H rows below this.
pub fn info_height(model: &Model) -> u16 {
    let mut h = 2u16;
    if model.now.title.is_some() {
        h += 1;
    }
    if model.notice.is_some() {
        h += 1;
    }
    h
}

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    let info_h = info_height(model);
    let rows = Layout::vertical([Constraint::Length(info_h), Constraint::Min(0)]).split(area);

    let top = Layout::horizontal([Constraint::Min(0), Constraint::Length(16)]).split(rows[0]);
    frame.render_widget(Paragraph::new(info_lines(model, pal)), top[0]);
    frame.render_widget(
        Paragraph::new(volume_label(model)).style(Style::default().fg(pal.accent).bold()),
        top[1],
    );
    render_spectrum(model, pal, frame, rows[1]);
}

fn info_lines(model: &Model, pal: &Palette) -> Vec<Line<'static>> {
    // line 1 — system info, muted: brand · sync · version · update indicator.
    let brand = Span::styled("▌WR · WORLD RADIO", Style::default().fg(pal.dim).bold());
    let sync_span = match model.synced() {
        true => Span::styled("  ⊙ synced", Style::default().fg(pal.accent)),
        false => Span::styled("  ○ local", Style::default().fg(pal.dim)),
    };
    let version_span = Span::styled(
        format!(" · v{}", radio_core::update::current_version()),
        Style::default().fg(pal.dim),
    );
    let update_span = match (&model.pending_update, model.update_applied) {
        (_, true) => Span::styled(
            "  ↑ press U to restart",
            Style::default().fg(pal.accent).bold(),
        ),
        (Some(rel), false) => Span::styled(
            format!("  ↑ v{} available", rel.version),
            Style::default().fg(pal.accent).bold(),
        ),
        (None, false) => Span::raw(""),
    };
    let system_line = Line::from(vec![brand, sync_span, version_span, update_span]);

    // line 2 — the content: status · station · meta, bright.
    let state = status_label(model, pal);
    let name = model
        .now
        .station_name
        .clone()
        .unwrap_or_else(|| "— idle —".to_string());
    let meta = format!(
        "{} · {} {}k",
        model.glyphs.country(&model.now.country),
        model.now.codec,
        model.now.bitrate
    );
    let content_line = Line::from(vec![
        state,
        Span::raw("  "),
        Span::styled(name, Style::default().fg(pal.fg).bold()),
        Span::raw("  "),
        Span::styled(meta, Style::default().fg(pal.dim)),
    ]);

    let mut lines = vec![system_line, content_line];
    if let Some(t) = &model.now.title {
        lines.push(Line::from(Span::styled(
            format!("♪ {t}"),
            Style::default().fg(pal.fg),
        )));
    }
    if let Some(n) = &model.notice {
        lines.push(Line::from(Span::styled(
            n.clone(),
            Style::default().fg(pal.peak),
        )));
    }
    lines
}

fn status_label(model: &Model, pal: &Palette) -> Span<'static> {
    use radio_core::audio::Status;
    match &model.status {
        Status::Playing { .. } => Span::styled(
            format!("{} LIVE", model.glyphs.live),
            Style::default().fg(pal.hot),
        ),
        Status::Buffering => Span::styled("◐ BUFFERING", Style::default().fg(pal.info)),
        Status::Retrying(n) => Span::styled(format!("◐ RETRY {n}"), Style::default().fg(pal.info)),
        Status::Error(_) => Span::styled("✗ ERROR", Style::default().fg(pal.err)),
        Status::Idle => Span::styled("■ IDLE", Style::default().fg(pal.dim)),
    }
}

fn volume_label(model: &Model) -> String {
    let pct = (model.volume.clamp(0.0, 1.0) * 100.0) as u16;
    let bars = (pct / 10).min(10) as usize;
    let filled = "█".repeat(bars);
    let empty = "░".repeat(10 - bars);
    format!("{filled}{empty} {pct:>3}%")
}

fn render_spectrum(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    if model.spectrum_style.is_off() {
        return;
    }
    if model.spectrum_bars.is_empty() || area.width == 0 || area.height == 0 {
        return;
    }
    let width = area.width as usize;
    let height = area.height as usize;
    let grid = render_grid(&model.spectrum_bars, width, height, model.spectrum_style);
    let last_row = height.saturating_sub(1).max(1) as f32;
    let lines: Vec<Line> = grid
        .iter()
        .enumerate()
        .map(|(row, cells)| {
            let up = 1.0 - row as f32 / last_row;
            let color = match up {
                _ if up > 0.75 => pal.hot,
                _ if up > 0.45 => pal.accent,
                _ if up > 0.15 => pal.info,
                _ => pal.ok,
            };
            let spans: Vec<Span> = cells
                .iter()
                .map(|cell| Span::styled(cell.glyph.to_string(), Style::default().fg(color)))
                .collect();
            Line::from(spans)
        })
        .collect();
    frame.render_widget(Paragraph::new(lines), area);
}
