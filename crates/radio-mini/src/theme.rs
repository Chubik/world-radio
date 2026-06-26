use egui::Color32;

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color32,
    pub panel: Color32,
    pub fg: Color32,
    pub hi: Color32,
    pub dim: Color32,
    pub rule: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub accent: Color32,
    pub bright: Color32,
    pub scan: f32,
    pub glow: Color32,
    pub light: bool,
}

impl Theme {
    pub fn amber() -> Self {
        Self {
            name: "amber-crt",
            bg: Color32::from_rgb(0x15, 0x10, 0x0b),
            panel: Color32::from_rgb(0x1b, 0x15, 0x10),
            fg: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            hi: Color32::from_rgb(0xff, 0xc4, 0x57),
            dim: Color32::from_rgb(0x6e, 0x54, 0x30),
            rule: Color32::from_rgb(0x3a, 0x2c, 0x17),
            ok: Color32::from_rgb(0x9e, 0xc0, 0x74),
            warn: Color32::from_rgb(0xff, 0xc4, 0x57),
            err: Color32::from_rgb(0xd9, 0x6a, 0x5a),
            accent: Color32::from_rgb(0xff, 0x8a, 0x3d),
            bright: Color32::from_rgb(0xff, 0xf0, 0xc0),
            scan: 0.16,
            glow: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            light: false,
        }
    }

    #[allow(dead_code)]
    pub fn nord() -> Self {
        Self {
            name: "nord",
            bg: Color32::from_rgb(0x2e, 0x34, 0x40),
            panel: Color32::from_rgb(0x3b, 0x42, 0x52),
            fg: Color32::from_rgb(0xd8, 0xde, 0xe9),
            hi: Color32::from_rgb(0x88, 0xc0, 0xd0),
            dim: Color32::from_rgb(0x4c, 0x56, 0x6a),
            rule: Color32::from_rgb(0x43, 0x4c, 0x5e),
            ok: Color32::from_rgb(0xa3, 0xbe, 0x8c),
            warn: Color32::from_rgb(0xeb, 0xcb, 0x8b),
            err: Color32::from_rgb(0xbf, 0x61, 0x6a),
            accent: Color32::from_rgb(0x81, 0xa1, 0xc1),
            bright: Color32::from_rgb(0xec, 0xef, 0xf4),
            scan: 0.10,
            glow: Color32::from_rgb(0x88, 0xc0, 0xd0),
            light: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn amber_is_default_and_dark() {
        let t = Theme::amber();
        assert_eq!(t.name, "amber-crt");
        assert!(t.bg.r() < 40 && t.bg.g() < 40 && t.bg.b() < 40);
    }

    #[test]
    fn alternate_differs_from_amber() {
        assert_ne!(Theme::amber().hi, Theme::nord().hi);
    }

    #[test]
    fn amber_has_full_design_tokens() {
        let t = Theme::amber();
        assert!(t.panel.r() < 60 && t.panel.g() < 60 && t.panel.b() < 60);
        assert!(t.bright.r() > 200 && t.bright.g() > 200);
        assert!(t.scan > 0.0 && t.scan < 0.5);
        assert!(!t.light);
    }
}
