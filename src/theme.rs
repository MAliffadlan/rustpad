use egui::{
    Align2, Button, Color32, Context, CornerRadius, CursorIcon, FontData, FontFamily, FontId,
    FontInsert, FontPriority, InsertFontFamily, Margin, Pos2, Rect, Response, RichText, Sense,
    Shadow, Stroke, Style, TextStyle, Theme, Ui, Vec2, Visuals,
};

#[derive(Clone, Copy, Debug)]
pub struct Palette {
    pub dark: bool,
    pub panel: Color32,
    pub panel_alt: Color32,
    pub surface: Color32,
    pub surface_hover: Color32,
    pub surface_active: Color32,
    pub editor_bg: Color32,
    pub gutter_bg: Color32,
    pub border: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub faint: Color32,
    pub accent: Color32,
    pub accent_hover: Color32,
    pub accent_text: Color32,
    pub selection: Color32,
    pub find_match: Color32,
    pub find_active: Color32,
    pub line_highlight: Color32,
    pub error: Color32,
}

pub const DARK: Palette = Palette {
    dark: true,
    panel: Color32::from_rgb(20, 22, 27),
    panel_alt: Color32::from_rgb(25, 28, 34),
    surface: Color32::from_rgb(32, 35, 43),
    surface_hover: Color32::from_rgb(41, 45, 55),
    surface_active: Color32::from_rgb(49, 54, 67),
    editor_bg: Color32::from_rgb(16, 18, 23),
    gutter_bg: Color32::from_rgb(13, 15, 19),
    border: Color32::from_rgb(42, 46, 56),
    text: Color32::from_rgb(232, 234, 240),
    muted: Color32::from_rgb(154, 160, 174),
    faint: Color32::from_rgb(26, 29, 35),
    accent: Color32::from_rgb(226, 178, 74),
    accent_hover: Color32::from_rgb(241, 199, 99),
    accent_text: Color32::from_rgb(28, 21, 4),
    selection: Color32::from_rgba_unmultiplied_const(226, 178, 74, 92),
    find_match: Color32::from_rgba_unmultiplied_const(226, 178, 74, 34),
    find_active: Color32::from_rgba_unmultiplied_const(226, 178, 74, 78),
    line_highlight: Color32::from_rgba_unmultiplied_const(255, 255, 255, 6),
    error: Color32::from_rgb(227, 109, 109),
};

pub const LIGHT: Palette = Palette {
    dark: false,
    panel: Color32::from_rgb(240, 235, 224),
    panel_alt: Color32::from_rgb(231, 225, 212),
    surface: Color32::from_rgb(250, 247, 241),
    surface_hover: Color32::from_rgb(237, 231, 218),
    surface_active: Color32::from_rgb(226, 218, 203),
    editor_bg: Color32::from_rgb(251, 248, 242),
    gutter_bg: Color32::from_rgb(243, 238, 228),
    border: Color32::from_rgb(215, 207, 192),
    text: Color32::from_rgb(43, 39, 33),
    muted: Color32::from_rgb(123, 116, 103),
    faint: Color32::from_rgb(232, 226, 214),
    accent: Color32::from_rgb(169, 118, 26),
    accent_hover: Color32::from_rgb(140, 97, 16),
    accent_text: Color32::from_rgb(255, 248, 232),
    selection: Color32::from_rgba_unmultiplied_const(169, 118, 26, 72),
    find_match: Color32::from_rgba_unmultiplied_const(169, 118, 26, 28),
    find_active: Color32::from_rgba_unmultiplied_const(169, 118, 26, 60),
    line_highlight: Color32::from_rgba_unmultiplied_const(0, 0, 0, 5),
    error: Color32::from_rgb(183, 70, 63),
};

pub fn palette(dark: bool) -> Palette {
    if dark { DARK } else { LIGHT }
}

pub const fn corner(radius: u8) -> CornerRadius {
    CornerRadius {
        nw: radius,
        ne: radius,
        sw: radius,
        se: radius,
    }
}

pub const fn margin(left: i8, right: i8, top: i8, bottom: i8) -> Margin {
    Margin {
        left,
        right,
        top,
        bottom,
    }
}

pub fn tint(color: Color32, alpha: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha)
}

pub fn install(ctx: &Context) {
    install_fonts(ctx);
    ctx.set_style_of(Theme::Dark, make_style(true));
    ctx.set_style_of(Theme::Light, make_style(false));
}

fn install_fonts(ctx: &Context) {
    const CANDIDATES: &[&str] = &[
        "/usr/share/fonts/truetype/noto/NotoSans-Regular.ttf",
        "/usr/share/fonts/truetype/liberation/LiberationSans-Regular.ttf",
        "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
    ];
    for path in CANDIDATES {
        if let Ok(bytes) = std::fs::read(path) {
            ctx.add_font(FontInsert {
                name: "Rustpad UI".to_owned(),
                data: FontData::from_owned(bytes),
                families: vec![InsertFontFamily {
                    family: FontFamily::Proportional,
                    priority: FontPriority::Highest,
                }],
            });
            break;
        }
    }
}

fn make_style(dark: bool) -> Style {
    let p = palette(dark);
    let mut style = Style {
        visuals: if dark {
            Visuals::dark()
        } else {
            Visuals::light()
        },
        ..Default::default()
    };

    macro_rules! widget {
        ($widget:expr, $bg:expr, $stroke:expr, $fg:expr) => {
            $widget.bg_fill = $bg;
            $widget.weak_bg_fill = $bg;
            $widget.bg_stroke = Stroke::new(1.0, $stroke);
            $widget.fg_stroke = Stroke::new(1.0, $fg);
            $widget.corner_radius = corner(8);
            $widget.expansion = 0.0;
        };
    }

    let shadow_alpha = if dark { 170 } else { 40 };
    let v = &mut style.visuals;
    v.panel_fill = p.editor_bg;
    v.window_fill = p.panel_alt;
    v.extreme_bg_color = p.surface;
    v.text_edit_bg_color = Some(p.surface);
    v.faint_bg_color = p.faint;
    v.code_bg_color = p.faint;
    v.override_text_color = Some(p.text);
    v.weak_text_color = Some(p.muted);
    v.weak_text_alpha = 0.8;
    v.hyperlink_color = p.accent;
    v.warn_fg_color = p.accent;
    v.error_fg_color = p.error;
    v.window_corner_radius = corner(12);
    v.menu_corner_radius = corner(10);
    v.window_stroke = Stroke::new(1.0, p.border);
    v.window_shadow = Shadow {
        offset: [0, 10],
        blur: 30,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha),
    };
    v.popup_shadow = Shadow {
        offset: [0, 6],
        blur: 20,
        spread: 0,
        color: Color32::from_black_alpha(shadow_alpha.saturating_sub(30)),
    };
    v.selection.bg_fill = p.selection;
    v.selection.stroke = Stroke::new(1.0, p.text);
    v.text_cursor.stroke = Stroke::new(2.0, p.accent);
    widget!(v.widgets.noninteractive, p.panel, p.border, p.text);
    widget!(v.widgets.inactive, p.surface, p.border, p.text);
    widget!(v.widgets.hovered, p.surface_hover, p.accent, p.text);
    widget!(v.widgets.active, p.accent, p.accent, p.accent_text);
    widget!(v.widgets.open, p.surface_active, p.accent, p.text);

    let s = &mut style.spacing;
    s.item_spacing = Vec2::new(8.0, 6.0);
    s.window_margin = margin(14, 14, 12, 12);
    s.button_padding = Vec2::new(11.0, 6.0);
    s.menu_margin = margin(10, 10, 8, 8);
    s.interact_size = Vec2::new(36.0, 28.0);
    s.text_edit_width = 260.0;
    s.menu_width = 262.0;
    s.menu_spacing = 2.0;
    s.indent = 18.0;
    s.scroll.floating = true;
    s.scroll.bar_width = 8.0;
    s.scroll.floating_width = 6.0;
    s.scroll.floating_allocated_width = 0.0;
    s.scroll.bar_inner_margin = 2.0;
    s.scroll.bar_outer_margin = 2.0;

    style.text_styles.insert(
        TextStyle::Small,
        FontId::new(11.0, FontFamily::Proportional),
    );
    style
        .text_styles
        .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
    style.text_styles.insert(
        TextStyle::Button,
        FontId::new(13.5, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Heading,
        FontId::new(24.0, FontFamily::Proportional),
    );
    style.text_styles.insert(
        TextStyle::Monospace,
        FontId::new(14.0, FontFamily::Monospace),
    );

    style
}

pub fn brand_mark(ui: &mut Ui, size: f32) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.rect_filled(rect, corner((size * 0.28).round() as u8), p.accent);
    let pad = size * 0.24;
    let line_height = (size * 0.075).max(1.5);
    for (index, fraction) in [0.58_f32, 0.42, 0.5].into_iter().enumerate() {
        let y = rect.top() + pad + index as f32 * size * 0.17;
        let line = Rect::from_min_size(
            Pos2::new(rect.left() + pad, y),
            Vec2::new(size * fraction, line_height),
        );
        painter.rect_filled(
            line,
            corner((line_height * 0.5).ceil() as u8),
            p.accent_text,
        );
    }
    response
}

pub fn logo_mark(ui: &mut Ui, size: f32) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let (rect, response) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter();
    painter.circle_filled(rect.center(), size * 0.62, tint(p.accent, 24));
    painter.circle_stroke(
        rect.center(),
        size * 0.62,
        Stroke::new(1.0, tint(p.accent, 70)),
    );
    let mark = rect.shrink(size * 0.14);
    painter.rect_filled(mark, corner((size * 0.2) as u8), p.accent);
    let pad = mark.width() * 0.24;
    let line_height = (mark.height() * 0.075).max(2.0);
    for (index, fraction) in [0.58_f32, 0.42, 0.5].into_iter().enumerate() {
        let y = mark.top() + pad + index as f32 * mark.height() * 0.17;
        let line = Rect::from_min_size(
            Pos2::new(mark.left() + pad, y),
            Vec2::new(mark.width() * fraction, line_height),
        );
        painter.rect_filled(line, corner((line_height * 0.5) as u8), p.accent_text);
    }
    response
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Icon {
    Search,
    Save,
    Theme,
    Close,
    Plus,
}

pub fn icon_button(ui: &mut Ui, icon: Icon, tooltip: &str, enabled: bool) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let sense = if enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(Vec2::new(32.0, 26.0), sense);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, tooltip));
    let hovered = enabled && response.hovered();
    let painter = ui.painter();
    if hovered {
        painter.rect_filled(rect, corner(7), p.surface_hover);
    }
    let color = if !enabled {
        tint(p.muted, 110)
    } else if hovered {
        p.accent
    } else {
        p.muted
    };
    let stroke = Stroke::new(1.6, color);
    let center = rect.center();
    match icon {
        Icon::Search => {
            painter.circle_stroke(center + Vec2::new(-1.0, -1.0), 5.0, stroke);
            painter.line_segment(
                [center + Vec2::new(2.5, 2.5), center + Vec2::new(7.0, 7.0)],
                stroke,
            );
        }
        Icon::Save => {
            painter.line_segment(
                [center + Vec2::new(0.0, -6.0), center + Vec2::new(0.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-4.0, 0.0), center + Vec2::new(0.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(4.0, 0.0), center + Vec2::new(0.0, 4.0)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-6.0, 7.0), center + Vec2::new(6.0, 7.0)],
                stroke,
            );
        }
        Icon::Theme => {
            if ui.visuals().dark_mode {
                let background = if hovered { p.surface_hover } else { p.panel };
                painter.circle_filled(center + Vec2::new(-0.8, 0.0), 6.0, color);
                painter.circle_filled(center + Vec2::new(3.0, -2.2), 5.2, background);
            } else {
                painter.circle_stroke(center, 4.5, stroke);
                for (dx, dy) in [
                    (0.0, -8.0),
                    (0.0, 8.0),
                    (-8.0, 0.0),
                    (8.0, 0.0),
                    (-5.6, -5.6),
                    (5.6, 5.6),
                    (-5.6, 5.6),
                    (5.6, -5.6),
                ] {
                    painter.line_segment(
                        [
                            center + Vec2::new(dx, dy) * 0.72,
                            center + Vec2::new(dx, dy),
                        ],
                        Stroke::new(1.3, color),
                    );
                }
            }
        }
        Icon::Close => {
            let s = 3.8;
            painter.line_segment(
                [center + Vec2::new(-s, -s), center + Vec2::new(s, s)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(-s, s), center + Vec2::new(s, -s)],
                stroke,
            );
        }
        Icon::Plus => {
            painter.line_segment(
                [center + Vec2::new(-5.0, 0.0), center + Vec2::new(5.0, 0.0)],
                stroke,
            );
            painter.line_segment(
                [center + Vec2::new(0.0, -5.0), center + Vec2::new(0.0, 5.0)],
                stroke,
            );
        }
    }
    let response = response.on_hover_text(tooltip);
    if enabled {
        response.on_hover_cursor(CursorIcon::PointingHand)
    } else {
        response
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TabAction {
    Activate,
    Close,
}

pub fn tab_pill(
    ui: &mut Ui,
    title: &str,
    active: bool,
    modified: bool,
    tooltip: &str,
) -> Option<TabAction> {
    let p = palette(ui.visuals().dark_mode);
    let font = FontId::new(13.0, FontFamily::Proportional);
    let text_color = if active { p.text } else { p.muted };
    let galley = ui
        .painter()
        .layout_no_wrap(title.to_owned(), font.clone(), text_color);

    let pad = 11.0;
    let close_width = 20.0;
    let dot_width = if modified { 9.0 } else { 0.0 };
    let width = galley.size().x + pad * 2.0 + close_width + dot_width;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 27.0), Sense::hover());
    let close_rect = Rect::from_center_size(
        Pos2::new(rect.right() - 12.0, rect.center().y),
        Vec2::splat(17.0),
    );
    let title_rect =
        Rect::from_min_max(rect.min, Pos2::new(close_rect.left() - 2.0, rect.bottom()));
    let title_response = ui
        .interact(title_rect, ui.scope_id().with("tab-title"), Sense::click())
        .on_hover_text(tooltip)
        .on_hover_cursor(CursorIcon::PointingHand);
    let close_response = ui
        .interact(close_rect, ui.scope_id().with("tab-close"), Sense::click())
        .on_hover_text("Close tab (Ctrl+W)")
        .on_hover_cursor(CursorIcon::PointingHand);
    title_response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Tab {title}"))
    });
    close_response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, "Close tab"));
    let hovered = title_response.hovered() || close_response.hovered();

    let fill = if active {
        tint(p.accent, if p.dark { 30 } else { 24 })
    } else if hovered {
        p.surface_hover
    } else {
        Color32::TRANSPARENT
    };
    let painter = ui.painter();
    if fill != Color32::TRANSPARENT {
        painter.rect_filled(rect, corner(7), fill);
    }
    if active {
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left() + 10.0, rect.bottom() - 2.0),
                Vec2::new((rect.width() - 20.0).max(0.0), 2.0),
            ),
            corner(1),
            p.accent,
        );
    }
    painter.galley(
        Pos2::new(rect.left() + pad, rect.center().y - galley.size().y * 0.5),
        galley.clone(),
        text_color,
    );
    if modified {
        painter.circle_filled(
            Pos2::new(rect.left() + pad + galley.size().x + 4.5, rect.center().y),
            2.4,
            p.accent,
        );
    }
    if close_response.hovered() {
        painter.rect_filled(
            close_rect,
            corner(5),
            tint(p.text, if p.dark { 22 } else { 16 }),
        );
    }
    let x_color = if close_response.hovered() || active {
        p.text
    } else {
        p.muted
    };
    let x = 3.6;
    let c = close_rect.center();
    painter.line_segment(
        [c + Vec2::new(-x, -x), c + Vec2::new(x, x)],
        Stroke::new(1.2, x_color),
    );
    painter.line_segment(
        [c + Vec2::new(-x, x), c + Vec2::new(x, -x)],
        Stroke::new(1.2, x_color),
    );

    if close_response.clicked() {
        return Some(TabAction::Close);
    }
    if title_response.middle_clicked() {
        return Some(TabAction::Close);
    }
    if title_response.clicked() {
        return Some(TabAction::Activate);
    }
    None
}

pub fn plus_button(ui: &mut Ui) -> bool {
    icon_button(ui, Icon::Plus, "New tab (Ctrl+N)", true).clicked()
}

fn style_accent_button(ui: &mut Ui, text: &str, min_width: f32, height: f32) -> Response {
    let p = palette(ui.visuals().dark_mode);
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.weak_bg_fill = p.accent;
        v.widgets.inactive.bg_fill = p.accent;
        v.widgets.inactive.bg_stroke = Stroke::NONE;
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.accent_text);
        v.widgets.hovered.weak_bg_fill = p.accent_hover;
        v.widgets.hovered.bg_fill = p.accent_hover;
        v.widgets.hovered.bg_stroke = Stroke::NONE;
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.accent_text);
        v.widgets.active.weak_bg_fill = p.accent_hover;
        v.widgets.active.bg_fill = p.accent_hover;
        v.widgets.active.bg_stroke = Stroke::NONE;
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.accent_text);
        ui.add(
            Button::new(RichText::new(text).color(p.accent_text).size(13.5))
                .min_size(Vec2::new(min_width, height))
                .corner_radius(corner(8)),
        )
    })
    .inner
}

pub fn accent_button(ui: &mut Ui, text: &str, min_width: f32) -> Response {
    style_accent_button(ui, text, min_width, 32.0)
}

pub fn accent_button_small(ui: &mut Ui, text: &str) -> Response {
    style_accent_button(ui, text, 0.0, 26.0)
}

pub fn secondary_button(ui: &mut Ui, text: &str, min_width: f32) -> Response {
    let p = palette(ui.visuals().dark_mode);
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.text);
        ui.add(
            Button::new(RichText::new(text).color(p.text).size(13.5))
                .min_size(Vec2::new(min_width, 32.0))
                .corner_radius(corner(8)),
        )
    })
    .inner
}

pub fn danger_button(ui: &mut Ui, text: &str, min_width: f32) -> Response {
    let p = palette(ui.visuals().dark_mode);
    ui.scope(|ui| {
        let v = ui.visuals_mut();
        v.widgets.inactive.weak_bg_fill = tint(p.error, if p.dark { 22 } else { 16 });
        v.widgets.inactive.bg_fill = tint(p.error, if p.dark { 22 } else { 16 });
        v.widgets.inactive.bg_stroke = Stroke::new(1.0, tint(p.error, 80));
        v.widgets.inactive.fg_stroke = Stroke::new(1.0, p.error);
        v.widgets.hovered.weak_bg_fill = tint(p.error, if p.dark { 44 } else { 30 });
        v.widgets.hovered.bg_fill = tint(p.error, if p.dark { 44 } else { 30 });
        v.widgets.hovered.bg_stroke = Stroke::new(1.0, tint(p.error, 130));
        v.widgets.hovered.fg_stroke = Stroke::new(1.0, p.error);
        v.widgets.active.weak_bg_fill = tint(p.error, if p.dark { 58 } else { 40 });
        v.widgets.active.bg_fill = tint(p.error, if p.dark { 58 } else { 40 });
        v.widgets.active.fg_stroke = Stroke::new(1.0, p.error);
        ui.add(
            Button::new(RichText::new(text).color(p.error).size(13.5))
                .min_size(Vec2::new(min_width, 32.0))
                .corner_radius(corner(8)),
        )
    })
    .inner
}

pub fn chip(ui: &mut Ui, text: &str) -> Response {
    let p = palette(ui.visuals().dark_mode);
    egui::Frame::new()
        .fill(p.surface)
        .corner_radius(corner(5))
        .inner_margin(margin(7, 7, 2, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(p.muted));
        })
        .response
}

pub fn chip_accent(ui: &mut Ui, text: &str) -> Response {
    let p = palette(ui.visuals().dark_mode);
    egui::Frame::new()
        .fill(tint(p.accent, if p.dark { 26 } else { 20 }))
        .corner_radius(corner(5))
        .inner_margin(margin(7, 7, 2, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).size(11.0).color(p.accent));
        })
        .response
}

pub fn list_row(ui: &mut Ui, title: &str, subtitle: &str) -> Response {
    let p = palette(ui.visuals().dark_mode);
    let width = ui.available_width();
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 48.0), Sense::hover());
    let response = ui
        .interact(
            rect,
            ui.scope_id().with(("recent", title, subtitle)),
            Sense::click(),
        )
        .on_hover_cursor(CursorIcon::PointingHand);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let painter = ui.painter();
    if response.hovered() {
        painter.rect_filled(rect, corner(8), p.surface_hover);
        painter.rect_filled(
            Rect::from_min_size(
                Pos2::new(rect.left(), rect.top() + 9.0),
                Vec2::new(2.5, rect.height() - 18.0),
            ),
            corner(1),
            p.accent,
        );
    }
    painter.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 8.0),
        Align2::LEFT_TOP,
        truncate(title, 42),
        FontId::new(13.2, FontFamily::Proportional),
        p.text,
    );
    painter.text(
        Pos2::new(rect.left() + 12.0, rect.top() + 27.0),
        Align2::LEFT_TOP,
        truncate(subtitle, 58),
        FontId::new(11.0, FontFamily::Proportional),
        p.muted,
    );
    let cx = rect.right() - 16.0;
    let cy = rect.center().y;
    painter.line_segment(
        [Pos2::new(cx - 2.0, cy - 3.0), Pos2::new(cx + 1.0, cy)],
        Stroke::new(1.3, p.muted),
    );
    painter.line_segment(
        [Pos2::new(cx + 1.0, cy), Pos2::new(cx - 2.0, cy + 3.0)],
        Stroke::new(1.3, p.muted),
    );
    response
}

fn truncate(text: &str, max_chars: usize) -> String {
    let count = text.chars().count();
    if count <= max_chars {
        return text.to_owned();
    }
    let skip = count.saturating_sub(max_chars.saturating_sub(1));
    let tail: String = text.chars().skip(skip).collect();
    format!("…{tail}")
}
