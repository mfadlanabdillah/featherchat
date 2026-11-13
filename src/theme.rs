use eframe::egui::{self, Color32, FontData, FontDefinitions, FontFamily};

#[derive(Clone, Copy)]
pub struct Palette {
    pub dark: bool,
    pub bg: Color32,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub header: Color32,
    pub hover: Color32,
    pub selected: Color32,
    pub border: Color32,
    pub text: Color32,
    pub weak: Color32,
    pub accent: Color32,
    pub accent_deep: Color32,
    pub accent_soft: Color32,
    pub outgoing: Color32,
    pub incoming: Color32,
    pub wallpaper: Color32,
    pub badge: Color32,
    pub tick: Color32,
    pub danger: Color32,
    pub rail: Color32,
    pub search_bg: Color32,
    pub shadow: Color32,
    pub overlay: Color32,
}

impl Palette {
    pub fn light() -> Self {
        Self {
            dark: false,
            bg: Color32::from_rgb(240, 237, 231),
            panel: Color32::from_rgb(255, 255, 255),
            panel_alt: Color32::from_rgb(247, 248, 250),
            header: Color32::from_rgb(247, 248, 250),
            hover: Color32::from_rgb(242, 244, 246),
            selected: Color32::from_rgb(237, 243, 242),
            border: Color32::from_rgb(231, 235, 237),
            text: Color32::from_rgb(17, 27, 33),
            weak: Color32::from_rgb(102, 119, 129),
            accent: Color32::from_rgb(24, 160, 110),
            accent_deep: Color32::from_rgb(8, 116, 79),
            accent_soft: Color32::from_rgb(226, 246, 238),
            outgoing: Color32::from_rgb(221, 248, 228),
            incoming: Color32::from_rgb(255, 255, 255),
            wallpaper: Color32::from_rgb(239, 235, 227),
            badge: Color32::from_rgb(24, 160, 110),
            tick: Color32::from_rgb(83, 189, 235),
            danger: Color32::from_rgb(214, 74, 74),
            rail: Color32::from_rgb(248, 249, 250),
            search_bg: Color32::from_rgb(240, 242, 245),
            shadow: Color32::from_rgba_unmultiplied(20, 30, 35, 22),
            overlay: Color32::from_rgba_unmultiplied(11, 20, 26, 90),
        }
    }

    pub fn dark() -> Self {
        Self {
            dark: true,
            bg: Color32::from_rgb(11, 20, 26),
            panel: Color32::from_rgb(17, 27, 33),
            panel_alt: Color32::from_rgb(24, 34, 41),
            header: Color32::from_rgb(31, 44, 51),
            hover: Color32::from_rgb(32, 44, 51),
            selected: Color32::from_rgb(42, 57, 66),
            border: Color32::from_rgb(42, 57, 66),
            text: Color32::from_rgb(233, 237, 239),
            weak: Color32::from_rgb(134, 150, 160),
            accent: Color32::from_rgb(0, 168, 132),
            accent_deep: Color32::from_rgb(6, 207, 156),
            accent_soft: Color32::from_rgb(20, 62, 55),
            outgoing: Color32::from_rgb(0, 92, 75),
            incoming: Color32::from_rgb(32, 44, 51),
            wallpaper: Color32::from_rgb(11, 20, 26),
            badge: Color32::from_rgb(0, 168, 132),
            tick: Color32::from_rgb(83, 189, 235),
            danger: Color32::from_rgb(240, 110, 110),
            rail: Color32::from_rgb(17, 27, 33),
            search_bg: Color32::from_rgb(32, 44, 51),
            shadow: Color32::from_rgba_unmultiplied(0, 0, 0, 70),
            overlay: Color32::from_rgba_unmultiplied(0, 0, 0, 140),
        }
    }

    pub fn apply(&self, ctx: &egui::Context) {
        let mut visuals = if self.dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        };
        visuals.panel_fill = self.panel;
        visuals.window_fill = self.panel;
        visuals.window_stroke = egui::Stroke::new(1.0_f32, self.border);
        visuals.extreme_bg_color = self.search_bg;
        visuals.faint_bg_color = self.panel_alt;
        visuals.override_text_color = Some(self.text);
        visuals.selection.bg_fill = self.accent;
        visuals.selection.stroke = egui::Stroke::new(1.0_f32, Color32::WHITE);
        visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0_f32, self.text);
        visuals.widgets.inactive.bg_fill = self.panel_alt;
        visuals.widgets.hovered.bg_fill = self.hover;
        visuals.widgets.active.bg_fill = self.selected;
        ctx.set_visuals(visuals);

        let mut style = (*ctx.style()).clone();
        style.spacing.item_spacing = egui::vec2(8.0, 8.0);
        style.spacing.button_padding = egui::vec2(10.0, 6.0);
        style.spacing.scroll.bar_width = 6.0;
        if let Some(font_id) = style.text_styles.get_mut(&egui::TextStyle::Body) {
            *font_id = egui::FontId::proportional(14.0);
        }
        ctx.set_style(style);
    }
}

pub fn font(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Proportional)
}

pub fn semibold(size: f32) -> egui::FontId {
    egui::FontId::new(size, FontFamily::Name("semibold".into()))
}

pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    let base = fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();

    let mut semibold_family = base.clone();

    let segoe = std::fs::read("C:/Windows/Fonts/segoeui.ttf").ok();
    let segoe_semibold = std::fs::read("C:/Windows/Fonts/seguisb.ttf").ok();

    if let Some(bytes) = segoe {
        fonts
            .font_data
            .insert("segoe".to_owned(), FontData::from_owned(bytes));
        if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
            list.insert(0, "segoe".to_owned());
        }
    }

    if let Some(bytes) = segoe_semibold {
        fonts.font_data.insert(
            "segoe_semibold".to_owned(),
            FontData::from_owned(bytes),
        );
        semibold_family.insert(0, "segoe_semibold".to_owned());
    }

    // Emoji and symbol fallbacks keep the little picker readable without
    // shipping a custom icon font.
    for (name, path) in [
        ("segoe_emoji", "C:/Windows/Fonts/seguiemj.ttf"),
        ("segoe_symbol", "C:/Windows/Fonts/seguisym.ttf"),
    ] {
        if let Ok(bytes) = std::fs::read(path) {
            fonts
                .font_data
                .insert(name.to_owned(), FontData::from_owned(bytes));
            if let Some(list) = fonts.families.get_mut(&FontFamily::Proportional) {
                list.push(name.to_owned());
            }
            semibold_family.push(name.to_owned());
        }
    }

    fonts
        .families
        .insert(FontFamily::Name("semibold".into()), semibold_family);

    ctx.set_fonts(fonts);
}
