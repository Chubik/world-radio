use crate::tui::keybind::Action;
use crate::tui::model::Model;
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let mut lines: Vec<Line> = vec![
        Line::styled("REMAP KEYS", Style::default().fg(pal.dim)),
        Line::from(""),
    ];
    for (i, action) in Action::ALL.iter().enumerate() {
        let selected = i == model.keybind_cursor;
        let chord = model.keymap.chord_for(*action).to_string_compact();
        let shown = match selected && model.keybind_capturing {
            true => "press a key…".to_string(),
            false => chord,
        };
        let marker = match selected {
            true => "▸ ",
            false => "  ",
        };
        let style = match selected {
            true => Style::default().fg(pal.peak).bold(),
            false => Style::default().fg(pal.fg),
        };
        lines.push(Line::styled(
            format!("{marker}{:<22}{shown}", action.label()),
            style,
        ));
    }
    lines.push(Line::from(""));
    if let Some(w) = &model.keybind_warning {
        lines.push(Line::styled(format!("⚠ {w}"), Style::default().fg(pal.hot)));
    }
    lines.push(Line::styled(
        "↑↓ move   ↵ rebind   r reset all   esc close",
        Style::default().fg(pal.dim),
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(pal.fg).bg(pal.bg))
            .block(
                Block::bordered()
                    .title("keybindings")
                    .style(Style::default().fg(pal.accent).bg(pal.bg)),
            ),
        area,
    );
}
