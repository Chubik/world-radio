use crate::tui::model::Model;
use crate::tui::theme::Palette;
use crate::tui::view::spectrum_render::render_grid;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub const SPECTRUM_H: u16 = 4;

/// height of the info block: a muted system row plus one content row that holds
/// status, station and the (scrolling) song title together. the notice, when set,
/// adds one more row. the spectrum, when on, is drawn in SPECTRUM_H rows below this.
pub fn info_height(model: &Model) -> u16 {
    let mut h = 2u16;
    if model.notice.is_some() {
        h += 1;
    }
    h
}

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    let info_h = info_height(model);
    let rows = Layout::vertical([Constraint::Length(info_h), Constraint::Min(0)]).split(area);

    // the info block spans the full width now that the volume meter is gone —
    // playback runs at full level and volume is controlled by the system mixer.
    frame.render_widget(
        Paragraph::new(info_lines(model, pal, rows[0].width)),
        rows[0],
    );
    render_spectrum(model, pal, frame, rows[1]);
}

fn info_lines(model: &Model, pal: &Palette, width: u16) -> Vec<Line<'static>> {
    // line 1 — system info, muted: brand · sync · version · meta · update indicator.
    let brand = Span::styled("▌WR · WORLD RADIO", Style::default().fg(pal.dim).bold());
    let sync_span = match model.synced() {
        true => Span::styled("  ⊙ synced", Style::default().fg(pal.accent)),
        false => Span::styled("  ○ local", Style::default().fg(pal.dim)),
    };
    let version_span = Span::styled(
        format!(" · v{}", radio_core::update::current_version()),
        Style::default().fg(pal.dim),
    );
    let meta_span = Span::styled(
        format!(
            " · {} {} {}k",
            model.glyphs.country(&model.now.country),
            model.now.codec,
            model.now.bitrate
        ),
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
    let system_line = Line::from(vec![brand, sync_span, version_span, meta_span, update_span]);

    // line 2 — status · station · ♪ song, all on one line. the song scrolls
    // (marquee) when it doesn't fit the remaining width.
    let state = status_label(model, pal);
    let name = model
        .now
        .station_name
        .clone()
        .unwrap_or_else(|| "— idle —".to_string());
    let mut content = vec![
        state.clone(),
        Span::raw("  "),
        Span::styled(name.clone(), Style::default().fg(pal.fg).bold()),
    ];
    if let Some(t) = &model.now.title {
        // width already spent by status + station + separators, roughly measured
        // from display widths so the marquee gets the true remaining columns.
        let used = state.content.chars().count() + 2 + name.chars().count() + 5;
        let avail = (width as usize).saturating_sub(used).max(8);
        let song = marquee(t, avail, model.spinner);
        content.push(Span::styled("  │  ", Style::default().fg(pal.dim)));
        content.push(Span::styled(
            format!("♪ {song}"),
            Style::default().fg(pal.fg),
        ));
    }
    let content_line = Line::from(content);

    let mut lines = vec![system_line, content_line];
    if let Some(n) = &model.notice {
        lines.push(Line::from(Span::styled(
            n.clone(),
            Style::default().fg(pal.peak),
        )));
    }
    lines
}

/// scroll `text` within `width` columns; when it fits, return it unchanged. when
/// it doesn't, slide a window across a padded loop of the text driven by `tick`.
fn marquee(text: &str, width: usize, tick: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }
    // a gap between the end and the wrapped start so it reads as a loop, not a jump.
    let gap = "   •   ";
    let mut loop_chars: Vec<char> = chars.clone();
    loop_chars.extend(gap.chars());
    let period = loop_chars.len();
    // advance one column every few ticks so it scrolls at a readable pace.
    let start = (tick / 3) % period;
    (0..width)
        .map(|i| loop_chars[(start + i) % period])
        .collect()
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

#[cfg(test)]
mod marquee_tests {
    use super::marquee;

    #[test]
    fn short_title_unchanged() {
        assert_eq!(marquee("Hard Rock", 20, 0), "Hard Rock");
    }

    #[test]
    fn long_title_scrolls_and_advances() {
        let long = "A Very Long Song Title That Cannot Possibly Fit In The Header Bar";
        let w = 20;
        let a = marquee(long, w, 0);
        let b = marquee(long, w, 30); // 10 columns later (tick/3)
        assert_eq!(a.chars().count(), w, "window is exactly width wide");
        assert_eq!(b.chars().count(), w);
        assert_ne!(a, b, "advancing the tick scrolls the window");
        // first frame starts at the title head
        assert!(a.starts_with("A Very Long"), "starts at title head: {a:?}");
    }

    #[test]
    fn wraps_around_without_panic() {
        let long = "0123456789ABCDEF";
        for tick in 0..500 {
            let out = marquee(long, 8, tick);
            assert_eq!(out.chars().count(), 8);
        }
    }
}
