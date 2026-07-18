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

/// how an option relates to the current filter — drives its colour, no marker.
#[derive(Clone, Copy, PartialEq)]
enum OptState {
    /// not selected
    Normal,
    /// filter is narrowed to this value ("show only")
    ShowOnly,
    /// this country is excluded (hidden everywhere)
    Hidden,
}

type FilterGroup = (&'static str, Vec<(String, OptState)>, bool);

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
    // colour legend, not markers: accent = showing only, red = hidden. when the
    // user is typing to jump to a country/tag, show the type-ahead buffer instead.
    let typing = matches!(active_group, 1 | 2) && !model.browse.filter_typeahead.is_empty();
    let hint_line = match (active_group, typing) {
        (_, true) => Line::from(vec![
            ratatui::text::Span::styled("type: ", Style::default().fg(pal.dim)),
            ratatui::text::Span::styled(
                model.browse.filter_typeahead.clone(),
                Style::default().fg(pal.peak).bold(),
            ),
            ratatui::text::Span::styled("  (⌫ clear)", Style::default().fg(pal.dim)),
        ]),
        (1, false) => Line::from(vec![
            ratatui::text::Span::styled("type to find · ↵ ", Style::default().fg(pal.dim)),
            ratatui::text::Span::styled("show only", Style::default().fg(pal.accent)),
            ratatui::text::Span::styled(" → ", Style::default().fg(pal.dim)),
            ratatui::text::Span::styled("exclude", Style::default().fg(pal.hot)),
            ratatui::text::Span::styled(" → off", Style::default().fg(pal.dim)),
        ]),
        (2, false) => Line::styled(
            "type to find · ↵ show only · ← → group",
            Style::default().fg(pal.dim),
        ),
        _ => Line::styled("← → switch group · ↵ apply", Style::default().fg(pal.dim)),
    };
    lines.push(hint_line);
    lines.push(Line::from(""));

    let (_, opts, _) = &groups[active_group];
    let sel = active_option.unwrap_or(0);

    // only long lists (countries, tags) get a multi-column grid to fill the width
    // and cut scrolling. short groups (status, codec, bitrate) stay a single
    // vertical column so the selected option is obvious and easy to move through.
    const GRID_MIN_OPTS: usize = 12;
    let cell_w = grid_cell_width(opts).max(1);
    let inner_w = width.saturating_sub(2); // panel borders
    let cols = match opts.len() > GRID_MIN_OPTS {
        true => (inner_w / cell_w).clamp(1, 6),
        false => 1,
    };
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
            let Some((label, state)) = opts.get(oi) else {
                break;
            };
            let focused = Some(oi) == active_option;
            let style = opt_style(pal, *state, focused);
            // in a grid, pad each cell to a uniform width so columns line up; in a
            // single column, keep the label tight so the cursor highlight hugs it.
            let text = match cols > 1 {
                true => format!("{label:<cell_w$}"),
                false => format!(" {label} "),
            };
            spans.push(ratatui::text::Span::styled(text, style));
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
    for (gi, (name, opts, _)) in groups.iter().enumerate() {
        let header_style = match Some(gi) == active_group {
            true => Style::default().fg(pal.accent).bold(),
            false => Style::default().fg(pal.dim),
        };
        lines.push(Line::styled(name.to_string(), header_style));
        for (oi, (label, state)) in opts.iter().enumerate() {
            let focused = Some(gi) == active_group && Some(oi) == active_option;
            let row_style = opt_style(pal, *state, focused);
            lines.push(Line::styled(label.clone(), row_style));
        }
        lines.push(Line::from(""));
    }
    lines
}

/// width of one grid cell: the longest label plus a two-space gutter, so every
/// column lines up and cells never touch.
fn grid_cell_width(opts: &[(String, OptState)]) -> usize {
    let longest = opts
        .iter()
        .map(|(label, _)| label.chars().count())
        .max()
        .unwrap_or(0);
    longest + 2
}

/// colour conveys the option's state — no checkbox markers. show-only is accent,
/// hidden is the hot (danger) colour, normal is the default foreground. the
/// cursor is drawn as an inverted block (bg fill) so "where am i" is unmistakable
/// regardless of the option's state colour.
fn opt_style(pal: &Palette, state: OptState, focused: bool) -> Style {
    let fg = match state {
        OptState::ShowOnly => pal.accent,
        OptState::Hidden => pal.hot,
        OptState::Normal => pal.fg,
    };
    match focused {
        true => Style::default().fg(pal.bg).bg(fg).bold(),
        false => match state {
            OptState::Normal => Style::default().fg(fg),
            _ => Style::default().fg(fg).bold(),
        },
    }
}

fn on(selected: bool) -> OptState {
    match selected {
        true => OptState::ShowOnly,
        false => OptState::Normal,
    }
}

fn status_options(model: &Model) -> Vec<(String, OptState)> {
    use crate::tui::model::StatusFilter;
    let s = model.browse.filters.status;
    vec![
        ("all".into(), on(s == StatusFilter::All)),
        ("favorites".into(), on(s == StatusFilter::Favorites)),
        ("recent".into(), on(s == StatusFilter::Recent)),
        ("blocked".into(), on(s == StatusFilter::Blocked)),
        ("dead".into(), on(s == StatusFilter::Dead)),
    ]
}

fn group_options(model: &Model, group: usize, facets: &[(String, u32)]) -> Vec<(String, OptState)> {
    let f = &model.browse.filters;
    let none_selected = match group {
        1 => f.countries.is_empty(),
        2 => f.tags.is_empty(),
        3 => f.codecs.is_empty(),
        _ => true,
    };
    let mut out = vec![("all".to_string(), on(none_selected))];
    for (v, count) in facets {
        let hidden = group == 1
            && model
                .browse
                .excluded_countries
                .iter()
                .any(|c| c.eq_ignore_ascii_case(v));
        // colour carries the state; the label stays clean with no markers.
        let state = match (hidden, f.group_selected(group, v)) {
            (true, _) => OptState::Hidden,
            (false, true) => OptState::ShowOnly,
            (false, false) => OptState::Normal,
        };
        out.push((format!("{v} ({count})"), state));
    }
    out
}

fn bitrate_options(current: Option<u32>) -> Vec<(String, OptState)> {
    vec![
        ("any".to_string(), on(current.is_none())),
        ("≥128 kbps".to_string(), on(current == Some(128))),
        ("≥256 kbps".to_string(), on(current == Some(256))),
        ("≥320 kbps".to_string(), on(current == Some(320))),
    ]
}

#[cfg(test)]
mod tests {

    #[test]
    fn grid_cell_width_is_longest_label_plus_gutter() {
        let opts = vec![
            ("US (7475)".to_string(), OptState::Normal),
            ("DE (6009)".to_string(), OptState::Normal),
        ];
        // longest label 9 chars + 2 (gutter) = 11
        assert_eq!(grid_cell_width(&opts), 11);
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
    fn opt_state_colour_conveys_show_only_vs_hidden() {
        // no markers — colour alone must distinguish the states.
        let pal = crate::tui::theme::Theme::AmberCrt.palette();
        assert_eq!(
            opt_style(&pal, OptState::ShowOnly, false).fg,
            Some(pal.accent)
        );
        assert_eq!(opt_style(&pal, OptState::Hidden, false).fg, Some(pal.hot));
        assert_eq!(opt_style(&pal, OptState::Normal, false).fg, Some(pal.fg));
        // the cursor is an inverted block: the state colour fills the background
        // and the text flips to the panel bg, so it's visible on any state.
        let cursor = opt_style(&pal, OptState::Hidden, true);
        assert_eq!(cursor.bg, Some(pal.hot));
        assert_eq!(cursor.fg, Some(pal.bg));
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
