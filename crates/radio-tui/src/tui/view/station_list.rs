use crate::tui::model::{BrowseFocus, Model, RowState};
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    let dim_focus = matches!(model.browse.focus, BrowseFocus::Filters { .. });
    let playing_uuid = model.now.uuid.as_deref();

    let viewport = area.height.saturating_sub(2) as usize;
    let total = model.browse.rows.len();
    let offset = centered_offset(model.browse.selected, total, viewport);
    let end = (offset + viewport).min(total);

    let items: Vec<ListItem> = model.browse.rows[offset..end]
        .iter()
        .enumerate()
        .map(|(vi, r)| {
            let i = offset + vi;
            row_line(
                model,
                pal,
                r,
                i == model.browse.selected,
                dim_focus,
                playing_uuid,
            )
        })
        .collect();

    let hide_tag = match model.browse.filters.hide_unplayable {
        true => "  · playable only",
        false => "",
    };
    let offline_tag = match model.browse.offline {
        true => "  · offline cache",
        false => "",
    };
    let header = match model.browse.searching_input {
        true => format!("SEARCH  ▌{}_", model.browse.query),
        false => format!(
            "SEARCH  {}   {} results · sort: {}{}{}",
            model.browse.query,
            model.browse.rows.len(),
            model.browse.sort.label(),
            hide_tag,
            offline_tag
        ),
    };
    if model.browse.rows.is_empty() {
        render_placeholder(model, pal, frame, area, &header);
        return;
    }
    let mut list_state = ListState::default();
    list_state.select(Some(model.browse.selected - offset));
    let list = List::new(items)
        .block(Block::bordered().title(header))
        .highlight_style(Style::default().bg(pal.accent).fg(pal.bg).bold());
    frame.render_stateful_widget(list, area, &mut list_state);
}

const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];

fn render_placeholder(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect, header: &str) {
    let (text, color) = match (model.browse.loading, model.browse.offline) {
        (true, _) => {
            let frame_glyph = SPINNER[(model.spinner / 2) % SPINNER.len()];
            (format!("{frame_glyph}  connecting…"), pal.info)
        }
        (false, true) => ("offline — nothing cached".to_string(), pal.dim),
        (false, false) => ("no stations found".to_string(), pal.dim),
    };
    let inner = area.height.saturating_sub(2);
    let pad = (inner / 2) as usize;
    let mut lines: Vec<Line> = vec![Line::from(""); pad];
    lines.push(Line::styled(
        format!("   {text}"),
        Style::default().fg(color).bold(),
    ));
    frame.render_widget(
        Paragraph::new(lines).block(Block::bordered().title(header.to_string())),
        area,
    );
}

fn centered_offset(selected: usize, total: usize, viewport: usize) -> usize {
    if viewport == 0 || total <= viewport {
        return 0;
    }
    let half = viewport / 2;
    let max_offset = total - viewport;
    selected.saturating_sub(half).min(max_offset)
}

use crate::tui::model::StationRow;
use crate::tui::theme::Glyphs;
use radio_core::audio::Status;

const NAME_W: usize = 24;
const META_W: usize = 9;

enum RowKind {
    Dead,
    Buffering,
    Retrying,
    Live,
    Selected,
    Normal,
}

fn row_kind(
    r: &StationRow,
    selected: bool,
    playing_uuid: Option<&str>,
    status: &Status,
) -> RowKind {
    if r.state == RowState::Disabled {
        return RowKind::Dead;
    }
    if playing_uuid == Some(r.uuid.as_str()) {
        return match status {
            Status::Buffering => RowKind::Buffering,
            Status::Retrying(_) => RowKind::Retrying,
            Status::Playing { .. } => RowKind::Live,
            _ => RowKind::Normal,
        };
    }
    match selected {
        true => RowKind::Selected,
        false => RowKind::Normal,
    }
}

fn prefix_glyph(kind: &RowKind, glyphs: &Glyphs) -> &'static str {
    match kind {
        RowKind::Dead => "✗",
        RowKind::Buffering | RowKind::Retrying => "⏳",
        RowKind::Live => glyphs.playing,
        RowKind::Selected => glyphs.sel,
        RowKind::Normal => glyphs.normal,
    }
}

fn kind_color(kind: &RowKind, dim_focus: bool, pal: &Palette) -> (ratatui::style::Color, bool) {
    match kind {
        RowKind::Dead => (pal.dim, false),
        RowKind::Buffering | RowKind::Retrying => (pal.info, false),
        RowKind::Live => (pal.accent, true),
        RowKind::Selected if dim_focus => (pal.fg, false),
        RowKind::Selected => (pal.peak, true),
        RowKind::Normal => (pal.fg, false),
    }
}

fn state_label(kind: &RowKind, r: &StationRow) -> Option<&'static str> {
    match kind {
        RowKind::Dead => Some("dead"),
        RowKind::Buffering => Some("buffering…"),
        RowKind::Retrying => Some("retrying…"),
        RowKind::Live => Some("live"),
        _ if r.unstable() => Some("unstable"),
        _ => None,
    }
}

fn fmt_tags(tags: &str) -> String {
    tags.split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" · ")
}

fn truncate(s: &str, w: usize) -> String {
    let count = s.chars().count();
    if count <= w {
        return format!("{s:<w$}");
    }
    let cut: String = s.chars().take(w.saturating_sub(1)).collect();
    format!("{cut}…")
}

fn row_line(
    model: &Model,
    pal: &Palette,
    r: &StationRow,
    selected: bool,
    dim_focus: bool,
    playing_uuid: Option<&str>,
) -> ListItem<'static> {
    let kind = row_kind(r, selected, playing_uuid, &model.status);
    let (color, bold) = kind_color(&kind, dim_focus, pal);
    let name_style = match bold {
        true => Style::default().fg(color).bold(),
        false => Style::default().fg(color),
    };
    let prefix = prefix_glyph(&kind, &model.glyphs);
    let prefix_style = match kind {
        RowKind::Dead => Style::default().fg(pal.err),
        RowKind::Buffering | RowKind::Retrying => Style::default().fg(pal.info),
        RowKind::Live => Style::default().fg(pal.accent).bold(),
        _ => Style::default().fg(pal.dim),
    };
    let fav = match r.favorite {
        true => model.glyphs.fav_on,
        false => " ",
    };
    let meta = format!("{} {}k", r.codec, r.bitrate);

    let mut spans = vec![
        Span::styled(format!("{prefix} "), prefix_style),
        Span::styled(format!("{fav} "), name_style),
        Span::styled(truncate(&r.name, NAME_W), name_style),
        Span::raw(" "),
        Span::styled(
            format!("{:<4}", model.glyphs.country(&r.country)),
            Style::default().fg(pal.dim),
        ),
        Span::raw("  "),
        Span::styled(format!("{meta:<META_W$}"), Style::default().fg(pal.dim)),
        Span::raw(" "),
        Span::styled(fmt_tags(&r.tags), Style::default().fg(pal.dim)),
    ];
    if let Some(label) = state_label(&kind, r) {
        let state_color = match kind {
            RowKind::Dead => pal.err,
            RowKind::Live => pal.accent,
            RowKind::Buffering | RowKind::Retrying => pal.info,
            _ => pal.hot,
        };
        spans.push(Span::styled(
            format!("  {label}"),
            Style::default().fg(state_color),
        ));
    }
    ListItem::new(Line::from(spans))
}

#[cfg(test)]
mod tests {
    use super::centered_offset;

    #[test]
    fn no_offset_when_list_fits_viewport() {
        assert_eq!(centered_offset(3, 5, 10), 0);
        assert_eq!(centered_offset(0, 0, 10), 0);
    }

    #[test]
    fn no_offset_when_viewport_zero() {
        assert_eq!(centered_offset(50, 100, 0), 0);
    }

    #[test]
    fn centers_selection_in_middle() {
        assert_eq!(centered_offset(50, 100, 10), 45);
    }

    #[test]
    fn clamps_to_top_near_start() {
        assert_eq!(centered_offset(2, 100, 10), 0);
    }

    #[test]
    fn clamps_to_bottom_near_end() {
        assert_eq!(centered_offset(99, 100, 10), 90);
    }

    #[test]
    fn visible_slice_invariant_keeps_selection_in_window() {
        // for a huge list, only `viewport` rows are ever built, and the
        // selection stays within [0, viewport) after subtracting the offset
        let total = 12_150;
        let viewport = 30;
        for selected in [0usize, 1, 15, 6000, 12_119, 12_149] {
            let offset = centered_offset(selected, total, viewport);
            let end = (offset + viewport).min(total);
            assert!(selected >= offset, "selected below window");
            assert!(selected < end, "selected above window");
            assert!(selected - offset < viewport, "local index out of viewport");
            assert!(end - offset <= viewport, "slice wider than viewport");
        }
    }
}
