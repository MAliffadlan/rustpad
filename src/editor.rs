use egui::text::CCursor;
use egui::{
    Align, Align2, Color32, FontId, Id, Margin, Pos2, Rect, ScrollArea, TextEdit, TextStyle, Ui,
    Vec2,
};
use egui_extras::syntax_highlighting::{self, CodeTheme, SyntectSettings};

use crate::document::{self, Tab, TabId};
use crate::theme;

#[derive(Clone, Copy, Debug)]
pub struct EditorInfo {
    pub line: usize,
    pub col: usize,
    pub selection_chars: usize,
    pub cursor_char: usize,
    pub changed: bool,
}

impl Default for EditorInfo {
    fn default() -> Self {
        Self {
            line: 1,
            col: 1,
            selection_chars: 0,
            cursor_char: 0,
            changed: false,
        }
    }
}

pub fn editor_id(tab_id: TabId) -> Id {
    Id::unique(("rustpad-editor", tab_id))
}

pub fn show(
    ui: &mut Ui,
    tab: &mut Tab,
    word_wrap: bool,
    highlight: &SyntectSettings,
    scroll_to_char: Option<usize>,
    find_matches: &[std::ops::Range<usize>],
    active_match: Option<&std::ops::Range<usize>>,
) -> EditorInfo {
    let id = editor_id(tab.id);
    let p = theme::palette(ui.visuals().dark_mode);
    let code_theme = CodeTheme::from_style(ui.style());
    let language = tab.lang.token;
    let highlighting = document::highlighting_enabled(&tab.text);
    let mut layouter = |ui: &Ui, buffer: &dyn egui::TextBuffer, wrap_width: f32| {
        let mut job = if highlighting {
            syntax_highlighting::highlight_with(
                ui.ctx(),
                ui.style(),
                &code_theme,
                buffer.as_str(),
                language,
                highlight,
            )
        } else {
            egui::text::LayoutJob::simple(
                buffer.as_str().to_owned(),
                TextStyle::Monospace.resolve(ui.style()),
                ui.visuals().text_color(),
                wrap_width,
            )
        };
        if !find_matches.is_empty() || active_match.is_some() {
            apply_find_highlights(
                &mut job,
                find_matches,
                active_match,
                p.find_match,
                p.find_active,
            );
        }
        job.wrap.max_width = if word_wrap { wrap_width } else { f32::INFINITY };
        ui.ctx().fonts_mut(|fonts| fonts.layout_job(job))
    };

    let digits = tab.line_index.line_count().to_string().len().max(3);
    let number_font = FontId::monospace(11.5);
    let sample =
        ui.painter()
            .layout_no_wrap("8".repeat(digits), number_font.clone(), Color32::WHITE);
    let gutter_width = (sample.size().x + 20.0).clamp(34.0, 120.0);
    let row_height = ui.text_style_height(&TextStyle::Monospace).max(1.0);

    let scroll = if word_wrap {
        ScrollArea::vertical()
    } else {
        ScrollArea::both()
    };

    let mut info = EditorInfo::default();
    scroll.auto_shrink([false, false]).show(ui, |ui| {
        let available_height = ui.available_height();
        let visible_rows = if available_height.is_finite() {
            ((available_height / row_height).ceil() as usize).clamp(3, 200)
        } else {
            30
        };
        let wrap_width = ui.available_width();
        let desired_width = if word_wrap { wrap_width } else { f32::INFINITY };

        let output = TextEdit::multiline(&mut tab.text)
            .id(id)
            .code_editor()
            .desired_width(desired_width)
            .desired_rows(visible_rows + 1)
            .background_color(p.editor_bg)
            .text_color(p.text)
            .margin(Margin {
                left: gutter_width as i8,
                right: 12,
                top: 12,
                bottom: 12,
            })
            .layouter(&mut layouter)
            .show(ui);

        let changed = output.response.changed();
        if changed {
            tab.mark_changed();
        }

        let cursor_range = output.cursor_range.or_else(|| {
            TextEdit::load_state(ui.ctx(), id).and_then(|state| state.cursor.char_range())
        });
        let (cursor_char, selection_chars) = match cursor_range {
            Some(range) => (
                range.primary.index.0,
                range.primary.index.0.abs_diff(range.secondary.index.0),
            ),
            None => (0, 0),
        };
        let (line, col) = tab.line_index.line_col(cursor_char);
        info = EditorInfo {
            line,
            col,
            selection_chars,
            cursor_char,
            changed,
        };

        paint_current_line(
            ui,
            &output.galley,
            output.galley_pos,
            output.text_clip_rect,
            gutter_width,
            line,
            p.line_highlight,
        );
        paint_gutter(
            ui,
            &output.galley,
            output.galley_pos,
            output.response.rect,
            gutter_width,
            &number_font,
            line,
            p,
        );

        if let Some(char_index) = scroll_to_char {
            let char_index = char_index.min(tab.text.chars().count());
            let cursor_rect = output
                .galley
                .pos_from_cursor(CCursor::new(char_index))
                .translate(output.galley_pos.to_vec2());
            ui.scroll_to_rect(
                cursor_rect.expand2(Vec2::new(0.0, 24.0)),
                Some(Align::Center),
            );
        }
    });

    info
}

fn paint_current_line(
    ui: &Ui,
    galley: &egui::Galley,
    galley_pos: Pos2,
    text_clip: Rect,
    gutter_width: f32,
    current_line: usize,
    color: Color32,
) {
    let clip = text_clip.intersect(ui.clip_rect());
    if clip.width() <= 0.0 || clip.height() <= 0.0 {
        return;
    }

    let mut line_number = 1usize;
    let mut top = None;
    let mut bottom = None;
    for placed in &galley.rows {
        let y = galley_pos.y + placed.pos.y;
        if line_number == current_line {
            top.get_or_insert(y);
            bottom = Some(y + placed.rect().height());
        } else if line_number > current_line {
            break;
        }
        if placed.ends_with_newline {
            line_number += 1;
        }
    }

    let (Some(top), Some(bottom)) = (top, bottom) else {
        return;
    };
    let left = clip.left() + gutter_width;
    let rect = Rect::from_min_max(
        Pos2::new(left, top.max(clip.top())),
        Pos2::new(clip.right(), bottom.min(clip.bottom())),
    );
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    ui.painter()
        .with_clip_rect(clip)
        .rect_filled(rect, theme::corner(2), color);
}

#[allow(clippy::too_many_arguments)]
fn paint_gutter(
    ui: &Ui,
    galley: &egui::Galley,
    galley_pos: Pos2,
    editor_rect: Rect,
    gutter_width: f32,
    font_id: &FontId,
    current_line: usize,
    p: theme::Palette,
) {
    let clip = ui.clip_rect();
    let top = editor_rect.top().max(clip.top());
    let bottom = editor_rect.bottom().min(clip.bottom());
    if bottom <= top {
        return;
    }

    let gutter_rect = Rect::from_min_max(
        Pos2::new(clip.left(), top),
        Pos2::new(clip.left() + gutter_width, bottom),
    );
    let painter = ui.painter().with_clip_rect(clip);
    painter.rect_filled(gutter_rect, 0.0, p.gutter_bg);
    painter.line_segment(
        [
            Pos2::new(gutter_rect.right(), top),
            Pos2::new(gutter_rect.right(), bottom),
        ],
        egui::Stroke::new(1.0, p.border),
    );
    let mut line_number = 1usize;

    for (index, placed) in galley.rows.iter().enumerate() {
        let y = galley_pos.y + placed.pos.y;
        if y > clip.bottom() {
            break;
        }
        let starts_logical_line = index == 0 || galley.rows[index - 1].ends_with_newline;
        if starts_logical_line && y + placed.rect().height() >= clip.top() {
            let is_current = line_number == current_line;
            if is_current {
                painter.rect_filled(
                    Rect::from_min_size(
                        Pos2::new(gutter_rect.left(), y),
                        Vec2::new(2.0, placed.rect().height()),
                    ),
                    theme::corner(1),
                    p.accent,
                );
            }
            painter.text(
                Pos2::new(gutter_rect.right() - 8.0, y + 1.0),
                Align2::RIGHT_TOP,
                line_number.to_string(),
                font_id.clone(),
                if is_current { p.accent } else { p.muted },
            );
        }
        if placed.ends_with_newline {
            line_number += 1;
        }
    }
}

pub(crate) const MAX_HIGHLIGHT_MATCHES: usize = 1_000;

fn apply_find_highlights(
    job: &mut egui::text::LayoutJob,
    matches: &[std::ops::Range<usize>],
    active: Option<&std::ops::Range<usize>>,
    normal: Color32,
    active_color: Color32,
) {
    use egui::text::{ByteIndex, LayoutSection};

    if matches.is_empty() && active.is_none() {
        return;
    }
    let text_len = job.text.len();
    let valid = |range: &std::ops::Range<usize>| {
        range.start < range.end
            && range.end <= text_len
            && job.text.is_char_boundary(range.start)
            && job.text.is_char_boundary(range.end)
    };

    let mut highlights: Vec<(std::ops::Range<usize>, Color32)> = Vec::new();
    let mut active_included = false;
    for range in matches.iter().take(MAX_HIGHLIGHT_MATCHES) {
        if !valid(range) {
            continue;
        }
        let is_active = active.is_some_and(|active| active == range);
        active_included |= is_active;
        highlights.push((range.clone(), if is_active { active_color } else { normal }));
    }
    if let Some(active) = active
        && valid(active)
        && !active_included
    {
        highlights.push((active.clone(), active_color));
    }
    if highlights.is_empty() {
        return;
    }
    highlights.sort_by_key(|(range, _)| range.start);

    let old_sections = std::mem::take(&mut job.sections);
    let mut new_sections = Vec::with_capacity(old_sections.len() + highlights.len() * 2);
    for section in old_sections {
        let section_start = section.byte_range.start.0;
        let section_end = section.byte_range.end.0;
        let mut boundaries = vec![section_start, section_end];
        for (range, _) in &highlights {
            if range.start > section_start && range.start < section_end {
                boundaries.push(range.start);
            }
            if range.end > section_start && range.end < section_end {
                boundaries.push(range.end);
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();

        let mut first = true;
        for pair in boundaries.windows(2) {
            let start = pair[0];
            let end = pair[1];
            if start >= end {
                continue;
            }
            let background = highlights
                .iter()
                .find(|(range, _)| range.start <= start && range.end >= end)
                .map(|(_, color)| *color);
            let mut format = section.format.clone();
            if let Some(color) = background {
                format.background = color;
            }
            new_sections.push(LayoutSection {
                leading_space: if first { section.leading_space } else { 0.0 },
                byte_range: ByteIndex(start)..ByteIndex(end),
                format,
            });
            first = false;
        }
    }
    job.sections = new_sections;
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::TextFormat;
    use egui::text::{ByteIndex, LayoutJob, LayoutSection};

    fn make_job(text: &str, sections: Vec<(std::ops::Range<usize>, Color32)>) -> LayoutJob {
        LayoutJob {
            text: text.to_owned(),
            sections: sections
                .into_iter()
                .map(|(range, color)| LayoutSection {
                    leading_space: 0.0,
                    byte_range: ByteIndex(range.start)..ByteIndex(range.end),
                    format: TextFormat {
                        color,
                        ..Default::default()
                    },
                })
                .collect(),
            ..Default::default()
        }
    }

    fn section_bgs(job: &LayoutJob) -> Vec<(usize, usize, Color32)> {
        job.sections
            .iter()
            .map(|section| {
                (
                    section.byte_range.start.0,
                    section.byte_range.end.0,
                    section.format.background,
                )
            })
            .collect()
    }

    #[test]
    fn splits_sections_and_paints_matches() {
        let normal = Color32::GREEN;
        let active = Color32::BLUE;
        let mut job = make_job("abcdefghij", vec![(0..10, Color32::RED)]);

        apply_find_highlights(&mut job, &[2..4, 7..9], Some(&(7..9)), normal, active);

        assert_eq!(
            section_bgs(&job),
            vec![
                (0, 2, Color32::TRANSPARENT),
                (2, 4, normal),
                (4, 7, Color32::TRANSPARENT),
                (7, 9, active),
                (9, 10, Color32::TRANSPARENT),
            ]
        );
        assert!(
            job.sections
                .iter()
                .all(|section| section.format.color == Color32::RED)
        );
    }

    #[test]
    fn highlights_across_multiple_sections() {
        let normal = Color32::GREEN;
        let mut job = make_job(
            "abcdefghij",
            vec![(0..5, Color32::RED), (5..10, Color32::BLUE)],
        );
        let match_range = 3usize..7usize;

        apply_find_highlights(
            &mut job,
            std::slice::from_ref(&match_range),
            None,
            normal,
            Color32::YELLOW,
        );

        assert_eq!(
            section_bgs(&job),
            vec![
                (0, 3, Color32::TRANSPARENT),
                (3, 5, normal),
                (5, 7, normal),
                (7, 10, Color32::TRANSPARENT),
            ]
        );
        assert_eq!(job.sections[0].format.color, Color32::RED);
        assert_eq!(job.sections[1].format.color, Color32::RED);
        assert_eq!(job.sections[2].format.color, Color32::BLUE);
        assert_eq!(job.sections[3].format.color, Color32::BLUE);
    }

    #[test]
    fn ignores_invalid_or_out_of_range_matches() {
        let mut job = make_job("aé", vec![(0..3, Color32::RED)]);

        apply_find_highlights(
            &mut job,
            &[2..3, 0..99],
            None,
            Color32::GREEN,
            Color32::BLUE,
        );

        assert_eq!(section_bgs(&job), vec![(0, 3, Color32::TRANSPARENT)]);
    }

    #[test]
    fn active_match_works_without_normal_matches() {
        let mut job = make_job("abcdefghij", vec![(0..10, Color32::RED)]);

        apply_find_highlights(&mut job, &[], Some(&(4..6)), Color32::GREEN, Color32::BLUE);

        assert_eq!(
            section_bgs(&job),
            vec![
                (0, 4, Color32::TRANSPARENT),
                (4, 6, Color32::BLUE),
                (6, 10, Color32::TRANSPARENT),
            ]
        );
    }

    #[test]
    fn no_highlights_leave_sections_untouched() {
        let mut job = make_job("abcdefghij", vec![(0..10, Color32::RED)]);
        let before = section_bgs(&job);

        apply_find_highlights(&mut job, &[], None, Color32::GREEN, Color32::BLUE);

        assert_eq!(section_bgs(&job), before);
    }
}
