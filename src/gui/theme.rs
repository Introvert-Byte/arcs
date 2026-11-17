use eframe::egui::{self, Color32, FontFamily, FontId, Rounding, Stroke, TextStyle, Visuals};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ThemeId {
    Light,
    Dark,
    Glass,
}

impl ThemeId {
    pub fn name(self) -> &'static str {
        match self {
            ThemeId::Light => "Light",
            ThemeId::Dark => "Dark",
            ThemeId::Glass => "Glass",
        }
    }

    pub fn all() -> &'static [ThemeId] {
        &[ThemeId::Light, ThemeId::Dark, ThemeId::Glass]
    }

    pub fn is_dark(self) -> bool {
        !matches!(self, ThemeId::Light)
    }
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub accent: Color32,
    pub accent_soft: Color32,
    pub accent_hover: Color32,
    pub muted: Color32,
    pub card: Color32,
    pub card_hover: Color32,
    pub border: Color32,
    pub warn: Color32,
    pub ok: Color32,
    pub bg: Color32,
    pub text: Color32,
    pub danger: Color32,
    pub sidebar: Color32,
    pub chip: Color32,
    pub chip_text: Color32,
    pub glow: Color32,
}

impl ThemeId {
    pub fn palette(self) -> Palette {
        match self {
            // Clean studio light
            ThemeId::Light => Palette {
                accent: Color32::from_rgb(79, 70, 229),
                accent_soft: Color32::from_rgba_unmultiplied(79, 70, 229, 22),
                accent_hover: Color32::from_rgb(67, 56, 202),
                muted: Color32::from_rgb(100, 116, 139),
                card: Color32::from_rgb(255, 255, 255),
                card_hover: Color32::from_rgb(248, 250, 252),
                border: Color32::from_rgb(226, 232, 240),
                warn: Color32::from_rgb(217, 119, 6),
                ok: Color32::from_rgb(5, 150, 105),
                bg: Color32::from_rgb(246, 247, 249),
                text: Color32::from_rgb(15, 23, 42),
                danger: Color32::from_rgb(220, 38, 38),
                sidebar: Color32::from_rgb(255, 255, 255),
                chip: Color32::from_rgb(236, 253, 245),
                chip_text: Color32::from_rgb(5, 150, 105),
                glow: Color32::from_rgba_unmultiplied(79, 70, 229, 40),
            },
            // Deep dark
            ThemeId::Dark => Palette {
                accent: Color32::from_rgb(129, 140, 248),
                accent_soft: Color32::from_rgba_unmultiplied(129, 140, 248, 28),
                accent_hover: Color32::from_rgb(165, 180, 252),
                muted: Color32::from_rgb(148, 163, 184),
                card: Color32::from_rgb(30, 41, 59),
                card_hover: Color32::from_rgb(51, 65, 85),
                border: Color32::from_rgb(51, 65, 85),
                warn: Color32::from_rgb(251, 191, 36),
                ok: Color32::from_rgb(52, 211, 153),
                bg: Color32::from_rgb(15, 23, 42),
                text: Color32::from_rgb(248, 250, 252),
                danger: Color32::from_rgb(248, 113, 113),
                sidebar: Color32::from_rgb(15, 23, 42),
                chip: Color32::from_rgb(6, 78, 59),
                chip_text: Color32::from_rgb(110, 231, 183),
                glow: Color32::from_rgba_unmultiplied(129, 140, 248, 50),
            },
            // Glass / liquid Project Lightning style
            ThemeId::Glass => Palette {
                accent: Color32::from_rgb(56, 189, 248), // cyan liquid
                accent_soft: Color32::from_rgba_unmultiplied(56, 189, 248, 36),
                accent_hover: Color32::from_rgb(125, 211, 252),
                muted: Color32::from_rgb(148, 163, 184),
                // Frosted glass panels (semi-transparent)
                card: Color32::from_rgba_unmultiplied(255, 255, 255, 28),
                card_hover: Color32::from_rgba_unmultiplied(255, 255, 255, 42),
                border: Color32::from_rgba_unmultiplied(255, 255, 255, 55),
                warn: Color32::from_rgb(251, 191, 36),
                ok: Color32::from_rgb(52, 211, 153),
                // Deep indigo-violet gradient base
                bg: Color32::from_rgb(12, 10, 28),
                text: Color32::from_rgb(241, 245, 249),
                danger: Color32::from_rgb(251, 113, 133),
                sidebar: Color32::from_rgba_unmultiplied(15, 12, 35, 200),
                chip: Color32::from_rgba_unmultiplied(56, 189, 248, 40),
                chip_text: Color32::from_rgb(125, 211, 252),
                glow: Color32::from_rgba_unmultiplied(167, 139, 250, 60),
            },
        }
    }
}

pub fn apply(ctx: &egui::Context, theme: ThemeId) {
    let p = theme.palette();
    let dark = theme.is_dark();
    let mut visuals = if dark {
        Visuals::dark()
    } else {
        Visuals::light()
    };

    visuals.panel_fill = p.bg;
    visuals.window_fill = if matches!(theme, ThemeId::Glass) {
        Color32::from_rgba_unmultiplied(20, 16, 40, 230)
    } else {
        p.card
    };
    visuals.extreme_bg_color = if matches!(theme, ThemeId::Glass) {
        Color32::from_rgb(8, 6, 22)
    } else if dark {
        Color32::from_rgb(2, 6, 23)
    } else {
        Color32::from_rgb(241, 245, 249)
    };
    visuals.faint_bg_color = p.card;
    visuals.widgets.noninteractive.bg_fill = p.card;
    visuals.widgets.inactive.bg_fill = p.card;
    visuals.widgets.hovered.bg_fill = p.card_hover;
    visuals.widgets.active.bg_fill = p.accent_soft;
    visuals.selection.bg_fill = p.accent_soft;
    visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.widgets.inactive.fg_stroke = Stroke::new(1.0_f32, p.text);
    visuals.window_stroke = Stroke::new(1.0_f32, p.border);
    visuals.override_text_color = Some(p.text);
    // Liquid rounded corners
    let r = if matches!(theme, ThemeId::Glass) { 16.0 } else { 12.0 };
    visuals.window_rounding = Rounding::same(r);
    visuals.menu_rounding = Rounding::same(r * 0.7);

    ctx.set_visuals(visuals);

    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing = egui::vec2(12.0, 10.0);
    style.spacing.button_padding = egui::vec2(16.0, 9.0);
    style.spacing.indent = 16.0;
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(22.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Body,
        FontId::new(14.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(14.0, FontFamily::Proportional),
    );
    ctx.set_style(style);
}
