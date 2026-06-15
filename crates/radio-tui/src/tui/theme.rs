use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    AmberCrt,
    TubeGlow,
    HifiPaper,
    ShortwaveGreen,
    CyberNeon,
    AtomicTerminal,
    MainframeBlue,
    Nord,
    Gruvbox,
    Dracula,
    Solarized,
    Catppuccin,
    RosePine,
    Monokai,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorTier {
    Truecolor,
    Ansi16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Palette {
    pub bg: Color,
    pub fg: Color,
    pub accent: Color,
    pub hot: Color,
    pub dim: Color,
    pub ok: Color,
    pub err: Color,
    pub info: Color,
    pub peak: Color,
}

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(r, g, b)
}

impl Theme {
    pub fn slug(self) -> &'static str {
        match self {
            Theme::AmberCrt => "amber-crt",
            Theme::TubeGlow => "tube-glow",
            Theme::HifiPaper => "hifi-paper",
            Theme::ShortwaveGreen => "shortwave-green",
            Theme::CyberNeon => "cyber-neon",
            Theme::AtomicTerminal => "atomic-terminal",
            Theme::MainframeBlue => "mainframe-blue",
            Theme::Nord => "nord",
            Theme::Gruvbox => "gruvbox",
            Theme::Dracula => "dracula",
            Theme::Solarized => "solarized",
            Theme::Catppuccin => "catppuccin",
            Theme::RosePine => "rose-pine",
            Theme::Monokai => "monokai",
        }
    }

    pub fn from_slug(s: &str) -> Theme {
        match s {
            "tube-glow" => Theme::TubeGlow,
            "hifi-paper" => Theme::HifiPaper,
            "shortwave-green" => Theme::ShortwaveGreen,
            "cyber-neon" => Theme::CyberNeon,
            "atomic-terminal" => Theme::AtomicTerminal,
            "mainframe-blue" => Theme::MainframeBlue,
            "nord" => Theme::Nord,
            "gruvbox" => Theme::Gruvbox,
            "dracula" => Theme::Dracula,
            "solarized" => Theme::Solarized,
            "catppuccin" => Theme::Catppuccin,
            "rose-pine" => Theme::RosePine,
            "monokai" => Theme::Monokai,
            _ => Theme::AmberCrt,
        }
    }

    pub fn next(self) -> Theme {
        match self {
            Theme::AmberCrt => Theme::TubeGlow,
            Theme::TubeGlow => Theme::HifiPaper,
            Theme::HifiPaper => Theme::ShortwaveGreen,
            Theme::ShortwaveGreen => Theme::CyberNeon,
            Theme::CyberNeon => Theme::AtomicTerminal,
            Theme::AtomicTerminal => Theme::MainframeBlue,
            Theme::MainframeBlue => Theme::Nord,
            Theme::Nord => Theme::Gruvbox,
            Theme::Gruvbox => Theme::Dracula,
            Theme::Dracula => Theme::Solarized,
            Theme::Solarized => Theme::Catppuccin,
            Theme::Catppuccin => Theme::RosePine,
            Theme::RosePine => Theme::Monokai,
            Theme::Monokai => Theme::AmberCrt,
        }
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::AmberCrt => Palette {
                bg: rgb(0x15, 0x10, 0x0B),
                fg: rgb(0xD4, 0x9A, 0x3A),
                accent: rgb(0xFF, 0xC4, 0x57),
                hot: rgb(0xFF, 0x8A, 0x3D),
                dim: rgb(0x6E, 0x54, 0x30),
                ok: rgb(0x9E, 0xC0, 0x74),
                err: rgb(0xD9, 0x6A, 0x5A),
                info: rgb(0x6F, 0xB0, 0xC8),
                peak: rgb(0xFF, 0xF0, 0xC0),
            },
            Theme::TubeGlow => Palette {
                bg: rgb(0x0B, 0x12, 0x20),
                fg: rgb(0xE5, 0xD7, 0xB8),
                accent: rgb(0xFF, 0xE3, 0xA8),
                hot: rgb(0xFF, 0x8A, 0x4D),
                dim: rgb(0x6A, 0x68, 0x55),
                ok: rgb(0x7F, 0xD9, 0xA8),
                err: rgb(0xFF, 0x6A, 0x6A),
                info: rgb(0x5C, 0xC7, 0xD8),
                peak: rgb(0xFF, 0xF2, 0xCC),
            },
            Theme::HifiPaper => Palette {
                bg: rgb(0xEF, 0xE6, 0xCC),
                fg: rgb(0x2E, 0x25, 0x17),
                accent: rgb(0xC5, 0x87, 0x2A),
                hot: rgb(0xA1, 0x3E, 0x2D),
                dim: rgb(0x8A, 0x7A, 0x5A),
                ok: rgb(0x5A, 0x7A, 0x3A),
                err: rgb(0xB1, 0x4D, 0x2D),
                info: rgb(0x2F, 0x66, 0x80),
                peak: rgb(0x0F, 0x0A, 0x04),
            },
            Theme::ShortwaveGreen => Palette {
                bg: rgb(0x06, 0x10, 0x08),
                fg: rgb(0x7F, 0xDA, 0x7F),
                accent: rgb(0xB5, 0xFF, 0x8A),
                hot: rgb(0xFF, 0x9D, 0x3D),
                dim: rgb(0x2D, 0x66, 0x33),
                ok: rgb(0x5F, 0xFF, 0x9C),
                err: rgb(0xFF, 0x5C, 0x5C),
                info: rgb(0x66, 0xC5, 0xFF),
                peak: rgb(0xD6, 0xFF, 0xC8),
            },
            Theme::CyberNeon => Palette {
                bg: rgb(0x07, 0x04, 0x1A),
                fg: rgb(0xC7, 0xC0, 0xE8),
                accent: rgb(0x00, 0xFF, 0xE1),
                hot: rgb(0xFF, 0x2B, 0xD5),
                dim: rgb(0x46, 0x38, 0x60),
                ok: rgb(0x6D, 0xFF, 0x7F),
                err: rgb(0xFF, 0x50, 0x50),
                info: rgb(0x5A, 0xD8, 0xFF),
                peak: rgb(0xFF, 0xFF, 0xFF),
            },
            Theme::AtomicTerminal => Palette {
                bg: rgb(0x0A, 0x1A, 0x0C),
                fg: rgb(0x4C, 0xDC, 0x60),
                accent: rgb(0x9C, 0xFF, 0x66),
                hot: rgb(0xFF, 0xC2, 0x32),
                dim: rgb(0x1F, 0x5E, 0x2A),
                ok: rgb(0x66, 0xFF, 0x5C),
                err: rgb(0xFF, 0x50, 0x40),
                info: rgb(0x5C, 0xFF, 0xAA),
                peak: rgb(0xD2, 0xFF, 0x8C),
            },
            Theme::MainframeBlue => Palette {
                bg: rgb(0x08, 0x1A, 0x3A),
                fg: rgb(0xD8, 0xE8, 0xFF),
                accent: rgb(0x66, 0xC0, 0xFF),
                hot: rgb(0xFF, 0xD5, 0x4A),
                dim: rgb(0x3A, 0x5A, 0x8A),
                ok: rgb(0x66, 0xE8, 0xA0),
                err: rgb(0xFF, 0x70, 0x70),
                info: rgb(0xFF, 0xB8, 0x4D),
                peak: rgb(0xFF, 0xFF, 0xFF),
            },
            Theme::Nord => Palette {
                bg: rgb(0x2E, 0x34, 0x40),
                fg: rgb(0xD8, 0xDE, 0xE9),
                accent: rgb(0x88, 0xC0, 0xD0),
                hot: rgb(0xD0, 0x87, 0x70),
                dim: rgb(0x4C, 0x56, 0x6A),
                ok: rgb(0xA3, 0xBE, 0x8C),
                err: rgb(0xBF, 0x61, 0x6A),
                info: rgb(0x81, 0xA1, 0xC1),
                peak: rgb(0xEC, 0xEF, 0xF4),
            },
            Theme::Gruvbox => Palette {
                bg: rgb(0x28, 0x28, 0x28),
                fg: rgb(0xEB, 0xDB, 0xB2),
                accent: rgb(0xFA, 0xBD, 0x2F),
                hot: rgb(0xFE, 0x80, 0x19),
                dim: rgb(0x66, 0x5C, 0x54),
                ok: rgb(0xB8, 0xBB, 0x26),
                err: rgb(0xFB, 0x49, 0x34),
                info: rgb(0x83, 0xA5, 0x98),
                peak: rgb(0xFB, 0xF1, 0xC7),
            },
            Theme::Dracula => Palette {
                bg: rgb(0x28, 0x2A, 0x36),
                fg: rgb(0xF8, 0xF8, 0xF2),
                accent: rgb(0xBD, 0x93, 0xF9),
                hot: rgb(0xFF, 0x79, 0xC6),
                dim: rgb(0x62, 0x72, 0xA4),
                ok: rgb(0x50, 0xFA, 0x7B),
                err: rgb(0xFF, 0x55, 0x55),
                info: rgb(0x8B, 0xE9, 0xFD),
                peak: rgb(0xF1, 0xFA, 0x8C),
            },
            Theme::Solarized => Palette {
                bg: rgb(0x00, 0x2B, 0x36),
                fg: rgb(0x93, 0xA1, 0xA1),
                accent: rgb(0x26, 0x8B, 0xD2),
                hot: rgb(0xCB, 0x4B, 0x16),
                dim: rgb(0x58, 0x6E, 0x75),
                ok: rgb(0x85, 0x99, 0x00),
                err: rgb(0xDC, 0x32, 0x2F),
                info: rgb(0x2A, 0xA1, 0x98),
                peak: rgb(0xFD, 0xF6, 0xE3),
            },
            Theme::Catppuccin => Palette {
                bg: rgb(0x1E, 0x1E, 0x2E),
                fg: rgb(0xCD, 0xD6, 0xF4),
                accent: rgb(0xCB, 0xA6, 0xF7),
                hot: rgb(0xF5, 0xC2, 0xE7),
                dim: rgb(0x6C, 0x70, 0x86),
                ok: rgb(0xA6, 0xE3, 0xA1),
                err: rgb(0xF3, 0x8B, 0xA8),
                info: rgb(0x89, 0xDC, 0xEB),
                peak: rgb(0xF9, 0xE2, 0xAF),
            },
            Theme::RosePine => Palette {
                bg: rgb(0x19, 0x17, 0x24),
                fg: rgb(0xE0, 0xDE, 0xF4),
                accent: rgb(0xC4, 0xA7, 0xE7),
                hot: rgb(0xEB, 0xBC, 0xBA),
                dim: rgb(0x6E, 0x6A, 0x86),
                ok: rgb(0x9C, 0xCF, 0xD8),
                err: rgb(0xEB, 0x6F, 0x92),
                info: rgb(0x31, 0x74, 0x8F),
                peak: rgb(0xF6, 0xC1, 0x77),
            },
            Theme::Monokai => Palette {
                bg: rgb(0x27, 0x28, 0x22),
                fg: rgb(0xF8, 0xF8, 0xF2),
                accent: rgb(0xA6, 0xE2, 0x2E),
                hot: rgb(0xF9, 0x26, 0x72),
                dim: rgb(0x75, 0x71, 0x5E),
                ok: rgb(0xA6, 0xE2, 0x2E),
                err: rgb(0xF9, 0x26, 0x72),
                info: rgb(0x66, 0xD9, 0xEF),
                peak: rgb(0xE6, 0xDB, 0x74),
            },
        }
    }
}

impl Palette {
    pub fn downgraded(self, tier: ColorTier) -> Palette {
        match tier {
            ColorTier::Truecolor => self,
            ColorTier::Ansi16 => Palette {
                bg: Color::Reset,
                fg: Color::White,
                accent: Color::Yellow,
                hot: Color::LightRed,
                dim: Color::DarkGray,
                ok: Color::Green,
                err: Color::Red,
                info: Color::Cyan,
                peak: Color::White,
            },
        }
    }
}

pub fn detect_tier() -> ColorTier {
    if std::env::var_os("NO_COLOR").is_some() {
        return ColorTier::Ansi16;
    }
    match std::env::var("COLORTERM") {
        Ok(v) if v.contains("truecolor") || v.contains("24bit") => ColorTier::Truecolor,
        _ => ColorTier::Ansi16,
    }
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Glyphs {
    pub playing: &'static str,
    pub stopped: &'static str,
    pub live: &'static str,
    pub fav_on: &'static str,
    pub fav_off: &'static str,
    pub sel: &'static str,
    pub normal: &'static str,
    pub bars: [&'static str; 8],
    pub sig_full: &'static str,
    pub sig_empty: &'static str,
    pub unstable: &'static str,
    pub emoji_flags: bool,
}

impl Glyphs {
    pub fn unicode() -> Glyphs {
        Glyphs {
            playing: "▶",
            stopped: "■",
            live: "●",
            fav_on: "★",
            fav_off: "☆",
            sel: "▸",
            normal: "·",
            bars: ["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"],
            sig_full: "●",
            sig_empty: "○",
            unstable: "⚠",
            emoji_flags: true,
        }
    }

    pub fn ascii() -> Glyphs {
        Glyphs {
            playing: ">",
            stopped: "[]",
            live: "(*)",
            fav_on: "*",
            fav_off: ".",
            sel: ">",
            normal: "·",
            bars: [" ", ".", ":", "-", "=", "+", "*", "#"],
            sig_full: "#",
            sig_empty: "-",
            unstable: "!",
            emoji_flags: false,
        }
    }

    pub fn for_config(no_emoji: bool) -> Glyphs {
        if no_emoji {
            let mut g = Glyphs::unicode();
            g.emoji_flags = false;
            g
        } else {
            Glyphs::unicode()
        }
    }

    pub fn country(&self, code: &str) -> String {
        let code = code.trim();
        if code.len() != 2 || !code.chars().all(|c| c.is_ascii_alphabetic()) {
            return "  ".to_string();
        }
        if self.emoji_flags {
            flag_emoji(code).unwrap_or_else(|| format!("[{}]", code.to_uppercase()))
        } else {
            format!("[{}]", code.to_uppercase())
        }
    }
}

fn flag_emoji(code: &str) -> Option<String> {
    let code = code.trim();
    match code.len() == 2 && code.chars().all(|c| c.is_ascii_alphabetic()) {
        false => None,
        true => Some(
            code.to_uppercase()
                .chars()
                .map(|c| char::from_u32(0x1F1E6 + (c as u32 - 'A' as u32)).unwrap())
                .collect(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_from_slug_defaults_to_amber_crt_on_unknown() {
        assert_eq!(Theme::from_slug("nonsense"), Theme::AmberCrt);
        assert_eq!(Theme::from_slug("cyber-neon"), Theme::CyberNeon);
    }

    #[test]
    fn amber_crt_palette_has_expected_bg() {
        let p = Theme::AmberCrt.palette();
        assert_eq!(p.bg, Color::Rgb(0x15, 0x10, 0x0B));
        assert_eq!(p.accent, Color::Rgb(0xFF, 0xC4, 0x57));
    }

    #[test]
    fn no_color_downgrade_maps_roles_to_named_colors() {
        let p = Theme::AmberCrt.palette().downgraded(ColorTier::Ansi16);
        assert_eq!(p.accent, Color::Yellow);
        assert_eq!(p.err, Color::Red);
    }

    #[test]
    fn glyphs_emoji_vs_ascii_favorite_marker() {
        assert_eq!(Glyphs::unicode().fav_on, "★");
        assert_eq!(Glyphs::ascii().fav_on, "*");
    }

    #[test]
    fn country_empty_is_blank() {
        assert_eq!(Glyphs::unicode().country(""), "  ");
        assert_eq!(Glyphs::unicode().country("USA"), "  ");
    }

    const ALL_THEMES: [Theme; 14] = [
        Theme::AmberCrt,
        Theme::TubeGlow,
        Theme::HifiPaper,
        Theme::ShortwaveGreen,
        Theme::CyberNeon,
        Theme::AtomicTerminal,
        Theme::MainframeBlue,
        Theme::Nord,
        Theme::Gruvbox,
        Theme::Dracula,
        Theme::Solarized,
        Theme::Catppuccin,
        Theme::RosePine,
        Theme::Monokai,
    ];

    #[test]
    fn theme_slug_roundtrips_all() {
        for t in ALL_THEMES {
            assert_eq!(Theme::from_slug(t.slug()), t);
        }
    }

    #[test]
    fn next_cycles_through_every_theme_once() {
        let mut seen = std::collections::HashSet::new();
        let mut t = Theme::AmberCrt;
        for _ in 0..ALL_THEMES.len() {
            assert!(seen.insert(t.slug()), "duplicate in cycle: {}", t.slug());
            t = t.next();
        }
        assert_eq!(t, Theme::AmberCrt, "cycle must return to start");
        assert_eq!(seen.len(), ALL_THEMES.len());
    }
}
