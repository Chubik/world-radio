use crate::tui::model::{BrowseFocus, Model};
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    let in_filters = matches!(model.browse.focus, BrowseFocus::Filters { .. });
    let hint = match (model.browse.searching_input, in_filters) {
        (true, _) => "type to filter   ^u clear   ↵ done   esc done",
        (false, true) => "↑↓ option   ←→ group   ↵ apply   c clear   C clear all   esc back",
        (false, false) => {
            "↑↓ select   ↵ play   r shuffle   o sort   f fav   B block   h hide   ⇥ filters   / search   , settings   ? help   q quit"
        }
    };
    frame.render_widget(
        Paragraph::new(hint).style(Style::default().fg(pal.dim)),
        area,
    );
}
