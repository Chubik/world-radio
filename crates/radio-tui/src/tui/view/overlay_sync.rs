use crate::tui::model::Model;
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let mut lines: Vec<Line> = Vec::new();
    match &model.sync_key {
        None => {
            lines.push(Line::from("○ local — not linked"));
            lines.push(Line::from(""));
            lines.push(Line::from("  [n] create key"));
            lines.push(Line::from("  or run: world-radio sync login"));
            lines.push(Line::from("  [esc] close"));
        }
        Some(key) => {
            lines.push(Line::from("⊙ synced"));
            lines.push(Line::from(""));
            lines.push(Line::from(format!("key: {key}")));
            lines.push(Line::from(""));
            for row in qr_rows(key) {
                lines.push(row);
            }
            lines.push(Line::from(""));
            lines.push(Line::from("  [c] copy   [r] re-sync"));
            lines.push(Line::from("  [l] log out   [d] delete account"));
            lines.push(Line::from("  [esc] close"));
        }
    }
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(pal.fg).bg(pal.bg))
            .block(
                Block::bordered()
                    .title("sync")
                    .style(Style::default().fg(pal.accent).bg(pal.bg)),
            ),
        area,
    );
}

fn qr_rows(key: &str) -> Vec<Line<'static>> {
    // medium error-correction and a full 4-module quiet zone — scanning a qr off a
    // monitor is unforgiving, and a 1-module margin with L-level correction is why
    // phone cameras fail to lock onto it.
    let code = match qrcode::QrCode::with_error_correction_level(key, qrcode::EcLevel::M) {
        Err(_) => return Vec::new(),
        Ok(c) => c,
    };
    let width = code.width();
    let quiet = 4;
    let side = width + quiet * 2;
    let dark: Vec<Vec<bool>> = code
        .to_colors()
        .chunks(width)
        .map(|row| {
            let mut padded = vec![false; quiet];
            padded.extend(row.iter().map(|c| *c == qrcode::Color::Dark));
            padded.extend(vec![false; quiet]);
            padded
        })
        .collect();
    let is_dark = |r: usize, c: usize| -> bool {
        match r < quiet || r >= width + quiet {
            true => false,
            false => dark[r - quiet][c],
        }
    };
    // draw each module as two blank spaces coloured by BACKGROUND, not with block
    // glyphs — block glyphs anti-alias to grey and leave gaps, which is why cameras
    // could not read it. spaces on a bg colour are clean solid squares. two spaces
    // per module keep the aspect roughly square against narrow terminal cells.
    let black = Style::default().bg(Color::Black);
    let white = Style::default().bg(Color::White);
    let mut rows = Vec::new();
    for r in 0..side {
        let mut spans: Vec<Span> = Vec::new();
        for c in 0..side {
            let style = match is_dark(r, c) {
                true => black,
                false => white,
            };
            spans.push(Span::styled("  ", style));
        }
        rows.push(Line::from(spans));
    }
    rows
}
