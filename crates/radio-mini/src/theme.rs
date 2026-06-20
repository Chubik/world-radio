use egui::Color32;

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    pub name: &'static str,
    pub bg: Color32,
    pub fg: Color32,
    pub hi: Color32,
    pub dim: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub accent: Color32,
}

impl Theme {
    pub fn amber() -> Self {
        Self {
            name: "amber-crt",
            bg: Color32::from_rgb(0x15, 0x10, 0x0b),
            fg: Color32::from_rgb(0xd4, 0x9a, 0x3a),
            hi: Color32::from_rgb(0xff, 0xc4, 0x57),
            dim: Color32::from_rgb(0x6e, 0x54, 0x30),
            ok: Color32::from_rgb(0x9e, 0xc0, 0x74),
            warn: Color32::from_rgb(0xff, 0xc4, 0x57),
            err: Color32::from_rgb(0xd9, 0x6a, 0x5a),
            accent: Color32::from_rgb(0xff, 0x8a, 0x3d),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "nord",
            bg: Color32::from_rgb(0x2e, 0x34, 0x40),
            fg: Color32::from_rgb(0xd8, 0xde, 0xe9),
            hi: Color32::from_rgb(0x88, 0xc0, 0xd0),
            dim: Color32::from_rgb(0x4c, 0x56, 0x6a),
            ok: Color32::from_rgb(0xa3, 0xbe, 0x8c),
            warn: Color32::from_rgb(0xeb, 0xcb, 0x8b),
            err: Color32::from_rgb(0xbf, 0x61, 0x6a),
            accent: Color32::from_rgb(0x81, 0xa1, 0xc1),
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
}
