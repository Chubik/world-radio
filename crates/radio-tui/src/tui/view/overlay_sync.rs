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
    match qrcode::QrCode::new(key) {
        Err(_) => Vec::new(),
        Ok(code) => code
            .render::<char>()
            .quiet_zone(true)
            .module_dimensions(2, 1)
            .build()
            .lines()
            .map(|l| l.to_string())
            .collect(),
    }
}
