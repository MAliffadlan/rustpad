use egui::{Align, Context, Key, Layout, Modal, Modifiers, RichText, Sense, TextEdit};

use crate::document::TabId;
use crate::theme;

pub enum Dialog {
    Error {
        title: String,
        message: String,
    },
    About,
    Quit {
        dirty: usize,
    },
    CloseUnsaved {
        tab: TabId,
        name: String,
    },
    OpenPath {
        path: String,
        focus: bool,
    },
    GoToLine {
        line: String,
        focus: bool,
        line_count: usize,
    },
}

pub enum DialogAction {
    Dismiss,
    QuitSaveAll,
    QuitDiscard,
    QuitCancel,
    SaveTab(TabId),
    DiscardTab(TabId),
    CancelCloseTab,
    OpenPath(String),
    GoToLine(usize),
}

pub fn show(ctx: &Context, dialog: &mut Dialog) -> Option<DialogAction> {
    let mut action = None;
    match dialog {
        Dialog::Error { title, message } => {
            let response = Modal::new(egui::Id::unique("rustpad-error-dialog")).show(ctx, |ui| {
                ui.set_max_width(440.0);
                dialog_title(ui, title);
                ui.label(RichText::new(message.as_str()).size(13.5));
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::accent_button(ui, "OK", 76.0).clicked() {
                        action = Some(DialogAction::Dismiss);
                    }
                });
            });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::Dismiss);
            }
        }
        Dialog::About => {
            let response = Modal::new(egui::Id::unique("rustpad-about-dialog")).show(ctx, |ui| {
                let p = theme::palette(ui.visuals().dark_mode);
                ui.set_max_width(430.0);
                ui.horizontal(|ui| {
                    let _ = theme::logo_mark(ui, 48.0);
                    ui.add_space(4.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("Rustpad").size(22.0).color(p.text));
                        ui.label(
                            RichText::new(format!("Versi {}", env!("CARGO_PKG_VERSION")))
                                .size(11.0)
                                .color(p.muted),
                        );
                    });
                });
                ui.add_space(12.0);
                ui.label(
                    RichText::new(
                        "Editor teks ringan untuk Linux, ditulis 100% dalam Rust dengan egui.",
                    )
                    .size(13.5)
                    .color(p.text),
                );
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    theme::chip_accent(ui, "Rust");
                    theme::chip(ui, "egui 0.37");
                    theme::chip(ui, "Linux native");
                });
                ui.add_space(14.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::secondary_button(ui, "Close", 80.0).clicked() {
                        action = Some(DialogAction::Dismiss);
                    }
                });
            });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::Dismiss);
            }
        }
        Dialog::Quit { dirty } => {
            let response = Modal::new(egui::Id::unique("rustpad-quit-dialog")).show(ctx, |ui| {
                ui.set_max_width(460.0);
                dialog_title(ui, "Keluar dari Rustpad?");
                ui.label(
                    RichText::new(format!(
                        "{dirty} tab memiliki perubahan yang belum disimpan."
                    ))
                    .size(13.5),
                );
                ui.add_space(12.0);
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if theme::accent_button(ui, "Save All", 96.0).clicked() {
                        action = Some(DialogAction::QuitSaveAll);
                    }
                    if theme::danger_button(ui, "Discard", 88.0).clicked() {
                        action = Some(DialogAction::QuitDiscard);
                    }
                    if theme::secondary_button(ui, "Cancel", 80.0).clicked() {
                        action = Some(DialogAction::QuitCancel);
                    }
                });
            });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::QuitCancel);
            }
        }
        Dialog::CloseUnsaved { tab, name } => {
            let tab = *tab;
            let response =
                Modal::new(egui::Id::unique(("rustpad-close-dialog", tab))).show(ctx, |ui| {
                    ui.set_max_width(460.0);
                    dialog_title(ui, "Simpan perubahan?");
                    ui.label(
                        RichText::new(format!(
                            "\"{name}\" memiliki perubahan yang belum disimpan."
                        ))
                        .size(13.5),
                    );
                    ui.add_space(12.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        if theme::accent_button(ui, "Save", 86.0).clicked() {
                            action = Some(DialogAction::SaveTab(tab));
                        }
                        if theme::danger_button(ui, "Discard", 88.0).clicked() {
                            action = Some(DialogAction::DiscardTab(tab));
                        }
                        if theme::secondary_button(ui, "Cancel", 80.0).clicked() {
                            action = Some(DialogAction::CancelCloseTab);
                        }
                    });
                });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::CancelCloseTab);
            }
        }
        Dialog::OpenPath { path, focus } => {
            let response =
                Modal::new(egui::Id::unique("rustpad-open-path-dialog")).show(ctx, |ui| {
                    let p = theme::palette(ui.visuals().dark_mode);
                    ui.set_max_width(540.0);
                    dialog_title(ui, "Buka path");
                    let field = ui.add(
                        TextEdit::singleline(path)
                            .hint_text("/path/ke/file.txt")
                            .desired_width(f32::INFINITY)
                            .background_color(p.surface)
                            .text_color(p.text),
                    );
                    if *focus {
                        field.request_focus();
                        *focus = false;
                    }
                    let submitted = (field.has_focus() || field.lost_focus())
                        && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
                    ui.add_space(12.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let open_clicked = theme::accent_button(ui, "Open", 82.0).clicked();
                        if (open_clicked || submitted) && !path.trim().is_empty() {
                            action = Some(DialogAction::OpenPath(path.clone()));
                        }
                        if theme::secondary_button(ui, "Cancel", 80.0).clicked() {
                            action = Some(DialogAction::Dismiss);
                        }
                    });
                });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::Dismiss);
            }
        }
        Dialog::GoToLine {
            line,
            focus,
            line_count,
        } => {
            let response =
                Modal::new(egui::Id::unique("rustpad-goto-line-dialog")).show(ctx, |ui| {
                    let p = theme::palette(ui.visuals().dark_mode);
                    ui.set_max_width(380.0);
                    dialog_title(ui, "Go to line");
                    let field = ui.add(
                        TextEdit::singleline(line)
                            .hint_text(format!("1–{line_count}"))
                            .desired_width(f32::INFINITY)
                            .background_color(p.surface)
                            .text_color(p.text),
                    );
                    if *focus {
                        field.request_focus();
                        *focus = false;
                    }
                    let parsed = parse_line_number(line, *line_count);
                    if !line.trim().is_empty() && parsed.is_none() {
                        ui.label(
                            RichText::new(format!("Masukkan nomor baris 1–{line_count}."))
                                .size(11.0)
                                .color(p.error),
                        );
                    }
                    let submitted = (field.has_focus() || field.lost_focus())
                        && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
                    ui.add_space(12.0);
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let go_clicked = theme::accent_button(ui, "Go", 76.0).clicked();
                        if let Some(line) = parsed
                            && (go_clicked || submitted)
                        {
                            action = Some(DialogAction::GoToLine(line));
                        }
                        if theme::secondary_button(ui, "Cancel", 80.0).clicked() {
                            action = Some(DialogAction::Dismiss);
                        }
                    });
                });
            if action.is_none() && response.should_close() {
                action = Some(DialogAction::Dismiss);
            }
        }
    }
    action
}

fn dialog_title(ui: &mut egui::Ui, title: &str) {
    let p = theme::palette(ui.visuals().dark_mode);
    ui.horizontal(|ui| {
        let _ = theme::brand_mark(ui, 22.0);
        ui.add_space(2.0);
        ui.label(RichText::new(title).size(19.0).color(p.text));
    });
    let (rect, _) = ui.allocate_exact_size(egui::vec2(36.0, 2.0), Sense::hover());
    ui.painter().rect_filled(rect, theme::corner(1), p.accent);
    ui.add_space(8.0);
}

pub(crate) fn parse_line_number(input: &str, line_count: usize) -> Option<usize> {
    let line: usize = input.trim().parse().ok()?;
    (1..=line_count).contains(&line).then_some(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_lines_within_range() {
        assert_eq!(parse_line_number("1", 10), Some(1));
        assert_eq!(parse_line_number("10", 10), Some(10));
        assert_eq!(parse_line_number("  7  ", 10), Some(7));
    }

    #[test]
    fn rejects_invalid_line_numbers() {
        assert_eq!(parse_line_number("0", 10), None);
        assert_eq!(parse_line_number("11", 10), None);
        assert_eq!(parse_line_number("-1", 10), None);
        assert_eq!(parse_line_number("abc", 10), None);
        assert_eq!(parse_line_number("", 10), None);
    }
}
