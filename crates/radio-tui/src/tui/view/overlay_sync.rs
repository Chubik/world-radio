use crate::tui::model::Model;
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
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
            for row in qr_lines(key) {
                lines.push(Line::from(row));
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
    let code = match qrcode::QrCode::with_error_correction_level(key, qrcode::EcLevel::L) {
        Err(_) => return Vec::new(),
        Ok(c) => c,
    };
    let width = code.width();
    let quiet = 1;
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
    let is_dark = |r: i64, c: usize| -> bool {
        match r < quiet as i64 || r >= (width + quiet) as i64 {
            true => false,
            false => dark[(r - quiet as i64) as usize][c],
        }
    };
    let mut lines = Vec::new();
    let mut r = 0i64;
    while r < side as i64 {
        let mut line = String::new();
        for c in 0..side {
            let top = is_dark(r, c);
            let bot = is_dark(r + 1, c);
            line.push(match (top, bot) {
                (true, true) => '█',
                (true, false) => '▀',
                (false, true) => '▄',
                (false, false) => ' ',
            });
        }
        lines.push(line);
        r += 2;
    }
    lines
}
