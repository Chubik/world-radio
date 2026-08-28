mod header;
mod hint;
mod overlay_help;
mod overlay_keybind;
mod overlay_settings;
mod overlay_sync;
mod sidebar;
mod spectrum_render;
mod station_list;

use crate::tui::model::{Model, Overlay};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::Block;
use ratatui::Frame;

pub fn view(model: &Model, frame: &mut Frame) {
    let pal = model.palette();
    let area = frame.area();
    frame.render_widget(
        Block::default().style(Style::default().bg(pal.bg).fg(pal.fg)),
        area,
    );

    let info_h = header::info_height(model);
    let header_h = match model.spectrum_style.is_off() {
        true => info_h,
        false => info_h + header::SPECTRUM_H,
    };
    let rows = Layout::vertical([
        Constraint::Length(header_h),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .split(area);

    header::render(model, &pal, frame, rows[0]);
    render_body(model, &pal, frame, rows[1]);
    hint::render(model, &pal, frame, rows[2]);

    match model.overlay {
        Overlay::None => {}
        Overlay::Settings => overlay_settings::render(model, &pal, frame, settings_box(area)),
        Overlay::Help => overlay_help::render(&pal, frame, centered(area)),
        Overlay::Keybindings => overlay_keybind::render(model, &pal, frame, centered(area)),
        Overlay::Sync => overlay_sync::render(model, &pal, frame, sync_box(area)),
    }
}

fn settings_box(area: Rect) -> Rect {
    let w = 44.min(area.width);
    let h = 12.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 3;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn sync_box(area: Rect) -> Rect {
    let w = 40.min(area.width);
    let h = 26.min(area.height);
    let x = area.x + (area.width.saturating_sub(w)) / 2;
    let y = area.y + (area.height.saturating_sub(h)) / 4;
    Rect {
        x,
        y,
        width: w,
        height: h,
    }
}

fn render_body(model: &Model, pal: &crate::tui::theme::Palette, frame: &mut Frame, area: Rect) {
    let in_filters = matches!(
        model.browse.focus,
        crate::tui::model::BrowseFocus::Filters { .. }
    );
    if !in_filters {
        station_list::render(model, pal, frame, area);
        return;
    }
    if area.width < 100 {
        let panel_h = sidebar::modal_height(model).min(area.height.saturating_sub(3));
        let rows = Layout::vertical([Constraint::Min(3), Constraint::Length(panel_h)]).split(area);
        station_list::render(model, pal, frame, rows[0]);
        sidebar::render_modal(model, pal, frame, rows[1]);
        return;
    }
    let cols = Layout::horizontal([Constraint::Length(25), Constraint::Min(0)]).split(area);
    sidebar::render(model, pal, frame, cols[0]);
    station_list::render(model, pal, frame, cols[1]);
}

fn centered(area: Rect) -> Rect {
    let v = Layout::vertical([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .split(area);
    let h = Layout::horizontal([
        Constraint::Percentage(10),
        Constraint::Percentage(80),
        Constraint::Percentage(10),
    ])
    .split(v[1]);
    h[1]
}
