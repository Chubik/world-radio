use crate::tui::model::Model;
use crate::tui::theme::Palette;
use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear, Paragraph};
use ratatui::Frame;

pub fn render(model: &Model, pal: &Palette, frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    let cursor = model.settings_cursor;
    let crossfade_mark = match model.crossfade {
        true => "[✓] on",
        false => "[ ] off",
    };
    let rows = [
        format!("theme        {}", theme_name(model.theme)),
        format!("crossfade    {crossfade_mark}"),
        format!("spectrum     {}", model.spectrum_style.label()),
        "keybindings  → edit".to_string(),
        format!("fft divisor  {:.1}", model.fft_divisor),
    ];
    let mut lines = vec![
        Line::styled("APPEARANCE", Style::default().fg(pal.dim)),
        Line::from(""),
    ];
    for (i, r) in rows.iter().enumerate() {
        let style = match i == cursor {
            true => Style::default().fg(pal.peak).bold(),
            false => Style::default().fg(pal.fg),
        };
        let marker = match i == cursor {
            true => "▸ ",
            false => "  ",
        };
        lines.push(Line::styled(format!("{marker}{r}"), style));
    }
    lines.push(Line::from(""));
    lines.push(Line::styled(
        "↕ move   ↵ toggle   ←→ adjust   esc close",
        Style::default().fg(pal.dim),
    ));
    frame.render_widget(
        Paragraph::new(lines)
            .style(Style::default().fg(pal.fg).bg(pal.bg))
            .block(
                Block::bordered()
                    .title("settings")
                    .style(Style::default().fg(pal.accent).bg(pal.bg)),
            ),
        area,
    );
}

fn theme_name(t: crate::tui::theme::Theme) -> &'static str {
    use crate::tui::theme::Theme;
    match t {
        Theme::AmberCrt => "amber crt",
        Theme::TubeGlow => "tube glow",
        Theme::HifiPaper => "hi-fi paper",
        Theme::ShortwaveGreen => "shortwave green",
        Theme::CyberNeon => "cyber neon",
        Theme::AtomicTerminal => "atomic terminal",
        Theme::MainframeBlue => "mainframe blue",
        Theme::Nord => "nord",
        Theme::Gruvbox => "gruvbox",
        Theme::Dracula => "dracula",
        Theme::Solarized => "solarized",
        Theme::Catppuccin => "catppuccin",
        Theme::RosePine => "rosé pine",
        Theme::Monokai => "monokai",
    }
}
