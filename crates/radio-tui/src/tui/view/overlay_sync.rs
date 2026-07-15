use crate::tui::model::Model;
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
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
            // the qr must be dark-on-light with real contrast regardless of theme —
            // rendering it in the accent colour on a dark bg is unscannable. force
            // black modules on a white ground so any phone camera locks onto it.
            let qr_style = Style::default()
                .fg(ratatui::style::Color::Black)
                .bg(ratatui::style::Color::White);
            for row in qr_lines(key) {
                lines.push(Line::from(Span::styled(row, qr_style)));
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

fn qr_lines(key: &str) -> Vec<String> {
    // medium error-correction and a full 4-module quiet zone — scanning a qr off
    // a monitor is unforgiving, and a 1-module margin with L-level correction is
    // why phone cameras fail to lock onto it.
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
    // one text row per module row, each module two full blocks wide — half-block
    // glyphs leave vertical line gaps between rows that break the scan; solid rows
    // keep the modules connected. two columns per module compensates for narrow cells.
    let mut lines = Vec::new();
    for r in 0..side {
        let mut line = String::new();
        for c in 0..side {
            let cell = match is_dark(r, c) {
                true => "██",
                false => "  ",
            };
            line.push_str(cell);
        }
        lines.push(line);
    }
    lines
}
