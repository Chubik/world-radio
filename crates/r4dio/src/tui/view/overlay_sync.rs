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
    let is_dark = |r: i64, c: usize| -> bool {
        match r < quiet as i64 || r >= (width + quiet) as i64 {
            true => false,
            false => dark[(r - quiet as i64) as usize][c],
        }
    };
    // half-block rendering: two vertically-stacked modules per character cell via
    // ▀ ▄ █, so a module stays roughly square against the ~1:2 terminal cell and the
    // whole code is compact. crucially, set BOTH fg and bg to exact rgb black/white
    // (not named colours, which map to the terminal palette where "white" is often
    // grey) so the finder patterns have the hard contrast a phone scanner needs.
    let fg_dark = Color::Rgb(0, 0, 0);
    let bg_light = Color::Rgb(255, 255, 255);
    // each output line covers two module rows (top = r, bottom = r+1).
    let mut rows = Vec::new();
    let mut r = 0i64;
    while r < side as i64 {
        let mut spans: Vec<Span> = Vec::new();
        for c in 0..side {
            let top = is_dark(r, c);
            let bot = is_dark(r + 1, c);
            // ▀ paints the top half in fg, bottom in bg; ▄ the reverse; █/space full.
            let (glyph, fg, bg) = match (top, bot) {
                (true, true) => ("█", fg_dark, bg_light),
                (true, false) => ("▀", fg_dark, bg_light),
                (false, true) => ("▄", fg_dark, bg_light),
                (false, false) => (" ", fg_dark, bg_light),
            };
            spans.push(Span::styled(glyph, Style::default().fg(fg).bg(bg)));
        }
        rows.push(Line::from(spans));
        r += 2;
    }
    rows
}
