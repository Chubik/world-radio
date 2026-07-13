use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(pal: &Palette, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let lines = vec![
        Line::from("PLAYBACK & LIST"),
        Line::from("  ↑↓ j k    move cursor"),
        Line::from("  ⇕ J K     page (10 rows)"),
        Line::from("  ↵         play station"),
        Line::from("  r         shuffle (random station)"),
        Line::from("  o         cycle sort (default/name/country/bitrate)"),
        Line::from("  s         stop"),
        Line::from("  f         toggle favorite"),
        Line::from("  B         blacklist / block"),
        Line::from("  R         recheck dead station"),
        Line::from("  y         sync panel (key · qr · log out)"),
        Line::from("  U         update (when available)"),
        Line::from("  [ ]       volume"),
        Line::from(""),
        Line::from("SYNC (across devices)"),
        Line::from("  y         open the sync panel"),
        Line::from("  header    ⊙ synced  ·  ○ local"),
        Line::from("  once linked, favourites and blocklist"),
        Line::from("  stay in sync across your devices."),
        Line::from("  scan the panel's qr in the phone app to pair."),
        Line::from(""),
        Line::from("FILTERS & STATUS"),
        Line::from("  ⇥ tab     filter focus on/off"),
        Line::from("  h l ←→    switch group (in filters)"),
        Line::from("  c / C     clear group / all"),
        Line::from("  /         search"),
        Line::from("  h         hide dead + unstable (in list)"),
        Line::from("  status: all · favorites · recent · blocked"),
        Line::from("  ✗ dead   ⚠ unstable (he-aac / aac+)"),
        Line::from(""),
        Line::from("OVERLAYS"),
        Line::from("  ,         settings (toggle)"),
        Line::from("  ?         help (toggle)"),
        Line::from("  esc / q   dismiss / quit"),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(pal.fg).bg(pal.bg))
            .block(
                Block::bordered()
                    .title("help · keybindings")
                    .style(Style::default().fg(pal.accent).bg(pal.bg)),
            ),
        area,
    );
}
