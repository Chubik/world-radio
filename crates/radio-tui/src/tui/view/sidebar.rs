use crate::tui::model::{BrowseFocus, Model};
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    let lines = build_lines(model, pal);
    let p = Paragraph::new(lines).block(Block::bordered().title("FILTERS"));
    frame.render_widget(p, area);
}

pub fn modal_height(model: &Model) -> u16 {
    let active_group = match model.browse.focus {
        BrowseFocus::Filters { group, .. } => group,
        BrowseFocus::Stations => 0,
    };
    let options = groups(model)[active_group].1.len() as u16;
    panel_height(options)
}

fn panel_height(option_count: u16) -> u16 {
    // 3 header rows (tabs, hint, spacer) + options + 2 border rows
    option_count + 5
}

pub fn render_modal(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(
        Block::default().style(Style::default().bg(pal.bg).fg(pal.fg)),
        area,
    );
    let lines = build_active_group_lines(model, pal);
    let p = Paragraph::new(lines).block(Block::bordered().title("FILTERS"));
    frame.render_widget(p, area);
}

type FilterGroup = (&'static str, Vec<(String, bool)>, bool);

fn groups(model: &Model) -> [FilterGroup; 5] {
    [
        ("STATUS", status_options(model), false),
        (
            "COUNTRY",
            group_options(model, 1, &model.browse.facets.countries),
            true,
        ),
        (
            "TAG",
            group_options(model, 2, &model.browse.facets.tags),
            true,
        ),
        (
            "CODEC",
            group_options(model, 3, &model.browse.facets.codecs),
            true,
        ),
        (
            "BITRATE",
            bitrate_options(model.browse.filters.bitrate_min),
            false,
        ),
    ]
}

fn build_active_group_lines(model: &Model, pal: &Palette) -> Vec<Line<'static>> {
    let groups = groups(model);
    let (active_group, active_option) = match model.browse.focus {
        BrowseFocus::Filters { group, option } => (group, Some(option)),
        BrowseFocus::Stations => (0, None),
    };

    let mut tabs: Vec<ratatui::text::Span<'static>> = Vec::new();
    for (gi, (name, _, _)) in groups.iter().enumerate() {
        let style = match gi == active_group {
            true => Style::default().fg(pal.accent).bold(),
            false => Style::default().fg(pal.dim),
        };
        if gi > 0 {
            tabs.push(ratatui::text::Span::styled(
                " · ",
                Style::default().fg(pal.dim),
            ));
        }
        tabs.push(ratatui::text::Span::styled(name.to_string(), style));
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    lines.push(Line::from(tabs));
    lines.push(Line::styled(
        "← → switch group",
        Style::default().fg(pal.dim),
    ));
    lines.push(Line::from(""));

    let (_, opts, multi) = &groups[active_group];
    for (oi, (label, selected)) in opts.iter().enumerate() {
        let marker = marker_for(*multi, oi == 0, *selected);
        let row_style = match Some(oi) == active_option {
            true => Style::default().fg(pal.peak).bold(),
            false => Style::default().fg(pal.fg),
        };
        lines.push(Line::styled(format!("{marker} {label}"), row_style));
    }
    lines
}

fn build_lines(model: &Model, pal: &Palette) -> Vec<Line<'static>> {
    let groups = groups(model);

    let (active_group, active_option) = match model.browse.focus {
        BrowseFocus::Filters { group, option } => (Some(group), Some(option)),
        BrowseFocus::Stations => (None, None),
    };

    let mut lines: Vec<Line<'static>> = Vec::new();
    for (gi, (name, opts, multi)) in groups.iter().enumerate() {
        let header_style = match Some(gi) == active_group {
            true => Style::default().fg(pal.accent).bold(),
            false => Style::default().fg(pal.dim),
        };
        lines.push(Line::styled(name.to_string(), header_style));
        for (oi, (label, selected)) in opts.iter().enumerate() {
            let marker = marker_for(*multi, oi == 0, *selected);
            let row_style = match (Some(gi) == active_group, Some(oi) == active_option) {
                (true, true) => Style::default().fg(pal.peak).bold(),
                _ => Style::default().fg(pal.fg),
            };
            lines.push(Line::styled(format!("{marker} {label}"), row_style));
        }
        lines.push(Line::from(""));
    }
    lines
}

fn marker_for(multi: bool, is_all: bool, selected: bool) -> &'static str {
    if multi && !is_all {
        return match selected {
            true => "[x]",
            false => "[ ]",
        };
    }
    match selected {
        true => "◉",
        false => "○",
    }
}

fn status_options(model: &Model) -> Vec<(String, bool)> {
    use crate::tui::model::StatusFilter;
    let s = model.browse.filters.status;
    vec![
        ("all".into(), s == StatusFilter::All),
        ("favorites".into(), s == StatusFilter::Favorites),
        ("recent".into(), s == StatusFilter::Recent),
        ("blocked".into(), s == StatusFilter::Blocked),
        ("dead".into(), s == StatusFilter::Dead),
    ]
}

fn group_options(model: &Model, group: usize, facets: &[(String, u32)]) -> Vec<(String, bool)> {
    let f = &model.browse.filters;
    let none_selected = match group {
        1 => f.countries.is_empty(),
        2 => f.tags.is_empty(),
        3 => f.codecs.is_empty(),
        _ => true,
    };
    let mut out = vec![("all".to_string(), none_selected)];
    for (v, count) in facets {
        out.push((format!("{v} ({count})"), f.group_selected(group, v)));
    }
    out
}

fn bitrate_options(current: Option<u32>) -> Vec<(String, bool)> {
    vec![
        ("any".to_string(), current.is_none()),
        ("≥128 kbps".to_string(), current == Some(128)),
        ("≥256 kbps".to_string(), current == Some(256)),
        ("≥320 kbps".to_string(), current == Some(320)),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn panel_height_fits_options_plus_chrome() {
        // 5 status options -> 5 options + 3 header rows + 2 borders = 10
        assert_eq!(panel_height(5), 10);
    }

    #[test]
    fn panel_height_grows_with_options() {
        assert!(panel_height(20) > panel_height(5));
    }
}
