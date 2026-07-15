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
    let lines = build_active_group_lines(model, pal, area.height as usize, area.width as usize);
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

fn build_active_group_lines(
    model: &Model,
    pal: &Palette,
    height: usize,
    width: usize,
) -> Vec<Line<'static>> {
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
    let hint = match active_group == 1 {
        true => "↵ show only [✓] · x hide ✕ · ← → group",
        false => "← → switch group · ↵ apply",
    };
    lines.push(Line::styled(hint, Style::default().fg(pal.dim)));
    lines.push(Line::from(""));

    let (_, opts, multi) = &groups[active_group];
    let sel = active_option.unwrap_or(0);

    // lay the options out in a grid so a long list (200+ countries) fills the
    // width instead of a single tall column the user must scroll forever. each
    // cell is a fixed width; the number of columns is whatever fits the panel.
    let cell_w = grid_cell_width(opts).max(1);
    let inner_w = width.saturating_sub(2); // panel borders
    let cols = (inner_w / cell_w).clamp(1, 6);
    let header_rows = lines.len() + 2; // tabs + hint + blank, plus border
    let grid_rows = height.saturating_sub(header_rows).max(1);
    let total_rows = opts.len().div_ceil(cols);
    // window vertically around the selected row so it stays on screen.
    let sel_row = sel / cols;
    let start_row = sel_row
        .saturating_sub(grid_rows / 2)
        .min(total_rows.saturating_sub(grid_rows));
    let end_row = (start_row + grid_rows).min(total_rows);

    if start_row > 0 {
        lines.push(Line::styled(
            format!("  ↑ {} more", start_row * cols),
            Style::default().fg(pal.dim),
        ));
    }
    for row in start_row..end_row {
        let mut spans: Vec<ratatui::text::Span<'static>> = Vec::new();
        for col in 0..cols {
            let oi = row * cols + col;
            let Some((label, selected)) = opts.get(oi) else {
                break;
            };
            let marker = marker_for(*multi, oi == 0, *selected);
            let hidden = label.contains('✕');
            let style = match (Some(oi) == active_option, hidden) {
                (true, _) => Style::default().fg(pal.peak).bold(),
                (false, true) => Style::default().fg(pal.hot),
                (false, false) => Style::default().fg(pal.fg),
            };
            let text = format!("{marker} {label}");
            // pad each cell to a uniform width so the columns line up.
            let padded = format!("{text:<cell_w$}");
            spans.push(ratatui::text::Span::styled(padded, style));
        }
        lines.push(Line::from(spans));
    }
    if end_row < total_rows {
        lines.push(Line::styled(
            format!("  ↓ {} more", (total_rows - end_row) * cols),
            Style::default().fg(pal.dim),
        ));
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

/// width of one grid cell: the longest "[m] label" plus a two-space gutter, so
/// every column lines up and cells never touch.
fn grid_cell_width(opts: &[(String, bool)]) -> usize {
    let longest = opts
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    // "[✓] " marker prefix (4) + label + 2-space gutter.
    longest + 4 + 2
}

fn marker_for(multi: bool, is_all: bool, selected: bool) -> &'static str {
    if multi && !is_all {
        // a filter selection means "show only these" — use a check, not [x], which
        // reads as "excluded/removed" and is easily confused with `x hide country`.
        return match selected {
            true => "[✓]",
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
        let excluded = group == 1
            && model
                .browse
                .excluded_countries
                .iter()
                .any(|c| c.eq_ignore_ascii_case(v));
        let label = match excluded {
            true => format!("{v} ({count})  ✕ hidden"),
            false => format!("{v} ({count})"),
        };
        out.push((label, f.group_selected(group, v)));
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

    #[test]
    fn grid_cell_width_accounts_for_marker_and_gutter() {
        let opts = vec![
            ("US (7475)".to_string(), false),
            ("DE (6009)".to_string(), false),
        ];
        // longest label 9 chars + 4 (marker) + 2 (gutter) = 15
        assert_eq!(grid_cell_width(&opts), 15);
    }

    #[test]
    fn country_grid_uses_multiple_columns_when_wide() {
        use crate::tui::model::BrowseFocus;
        let mut m = Model::new(
            crate::tui::theme::Theme::AmberCrt,
            crate::tui::theme::ColorTier::Truecolor,
            crate::tui::theme::Glyphs::unicode(),
        );
        m.browse.facets.countries = (0..40).map(|i| (format!("C{i:02}"), 100u32)).collect();
        m.browse.focus = BrowseFocus::Filters {
            group: 1,
            option: 1,
        };
        // wide + short panel: must wrap into several columns, not 40 tall rows.
        let lines =
            build_active_group_lines(&m, &crate::tui::theme::Theme::AmberCrt.palette(), 12, 120);
        // header (tabs+hint+blank)=3; the rest are grid rows, far fewer than 40.
        let grid_line_count = lines.len() - 3;
        assert!(
            grid_line_count < 15,
            "expected wrapped grid, got {grid_line_count} rows"
        );
    }

    #[test]
    fn filter_selection_marker_is_check_not_x() {
        // an included ("show only") country must read as a check, not [x] —
        // [x] reads as excluded and caused users to confuse it with `x hide`.
        assert_eq!(marker_for(true, false, true), "[✓]");
        assert_eq!(marker_for(true, false, false), "[ ]");
    }

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
