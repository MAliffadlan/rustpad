use std::collections::VecDeque;
use std::path::{Path, PathBuf};

use egui::text::{CCursor, CCursorRange};
use egui::{
    Align, Button, CentralPanel, Color32, Context, Key, Layout, Modifiers, Panel, RichText,
    ScrollArea, Stroke, TextEdit, ThemePreference, Ui, ViewportCommand,
};
use egui_extras::syntax_highlighting::SyntectSettings;
use syntect::highlighting::ThemeSet;

use crate::config::{self, Config, ThemePref};
use crate::dialogs::{self, Dialog, DialogAction};
use crate::document::{self, Tab, TabId};
use crate::editor::{self, EditorInfo};
use crate::file_dialog::{DialogPurpose, DialogResult, FileDialogService};
use crate::find::FindState;
use crate::text_index;
use crate::theme;

#[derive(Clone, Debug)]
enum Action {
    NewTab,
    Open,
    OpenPathDialog,
    OpenRecent(PathBuf),
    Save,
    SaveAs,
    CloseActiveTab,
    CloseTab(TabId),
    ActivateTab(TabId),
    NextTab,
    PrevTab,
    GoToLineDialog,
    Quit,
    Find,
    Replace,
    FindNext,
    FindPrev,
    CloseFind,
    ReplaceCurrent,
    ReplaceAll,
    SetWrap(bool),
    SetTheme(ThemePref),
    CycleTheme,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    About,
}

#[derive(Clone, Copy)]
enum SaveKind {
    Plain,
    CloseAfter,
    QuitAfter,
}

#[derive(Default)]
struct QuitState {
    active: bool,
    saving: bool,
    queue: VecDeque<TabId>,
}

pub struct RustpadApp {
    tabs: Vec<Tab>,
    active: Option<TabId>,
    next_id: TabId,
    config: Config,
    config_path: Option<PathBuf>,
    find: FindState,
    dialog: Option<Dialog>,
    fd: FileDialogService,
    quit: QuitState,
    highlight: SyntectSettings,
    info: EditorInfo,
    pending_focus: Option<TabId>,
    deferred_focus: Option<TabId>,
    pending_scroll: Option<usize>,
    closing: bool,
    window_title: String,
}

impl RustpadApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        cli_paths: Vec<PathBuf>,
        mut config: Config,
        config_path: Option<PathBuf>,
    ) -> Self {
        config.prune_recent();
        theme::install(&cc.egui_ctx);
        apply_theme(&cc.egui_ctx, config.theme);

        let highlight = SyntectSettings {
            ps: two_face::syntax::extra_newlines(),
            ts: ThemeSet::load_defaults(),
        };

        let mut app = Self {
            tabs: Vec::new(),
            active: None,
            next_id: 0,
            config,
            config_path,
            find: FindState::default(),
            dialog: None,
            fd: FileDialogService::default(),
            quit: QuitState::default(),
            highlight,
            info: EditorInfo::default(),
            pending_focus: None,
            deferred_focus: None,
            pending_scroll: None,
            closing: false,
            window_title: "Rustpad".to_owned(),
        };

        if cli_paths.is_empty() {
            let session_paths = app.config.session.open_files.clone();
            for path in session_paths.into_iter().take(10) {
                if path.is_file() {
                    app.open_path(path, false, false);
                }
            }
            if let Some(active_path) = app.config.session.active.clone()
                && let Some(id) = app
                    .tabs
                    .iter()
                    .find(|tab| {
                        tab.path
                            .as_deref()
                            .is_some_and(|path| paths_equal(path, &active_path))
                    })
                    .map(|tab| tab.id)
            {
                app.active = Some(id);
            }
        } else {
            for path in cli_paths {
                app.open_path(path, true, true);
            }
        }

        if app.active.is_none() {
            app.active = app.tabs.first().map(|tab| tab.id);
        }
        app.pending_focus = app.active;
        app
    }

    fn active_tab(&self) -> Option<&Tab> {
        let id = self.active?;
        self.tabs.iter().find(|tab| tab.id == id)
    }

    fn active_index(&self) -> Option<usize> {
        let id = self.active?;
        self.tabs.iter().position(|tab| tab.id == id)
    }

    fn activate_tab(&mut self, id: TabId) {
        if !self.tabs.iter().any(|tab| tab.id == id) {
            return;
        }
        self.active = Some(id);
        self.deferred_focus = Some(id);
        self.pending_scroll = None;
        if self.find.open {
            self.recompute_find();
        }
    }

    fn cycle_tab(&mut self, delta: isize) {
        if self.tabs.is_empty() {
            return;
        }
        let current = self.active_index().unwrap_or(0);
        if let Some(index) = cycle_index(current, self.tabs.len(), delta) {
            let id = self.tabs[index].id;
            self.activate_tab(id);
        }
    }

    fn jump_to_line(&mut self, ctx: &Context, line: usize) {
        let Some(index) = self.active_index() else {
            return;
        };
        let id = self.tabs[index].id;
        let Some(char_index) = self.tabs[index].line_index.line_start_char(line) else {
            return;
        };
        self.set_selection(ctx, id, char_index, char_index);
        self.pending_scroll = Some(char_index);
    }

    fn new_tab(&mut self) {
        self.next_id += 1;
        let id = self.next_id;
        self.tabs.push(Tab::untitled(id));
        self.active = Some(id);
        self.pending_focus = Some(id);
        self.pending_scroll = None;
        self.persist_config();
        if self.find.open {
            self.recompute_find();
        }
    }

    fn open_path(&mut self, path: PathBuf, create_if_missing: bool, report_errors: bool) {
        let path = absolute_path(path);
        if path.is_dir() {
            if report_errors {
                self.show_error(
                    "Tidak bisa membuka folder",
                    format!("{} adalah folder, bukan file.", path.display()),
                );
            }
            return;
        }

        if let Some(id) = self
            .tabs
            .iter()
            .find(|tab| {
                tab.path
                    .as_deref()
                    .is_some_and(|open_path| paths_equal(open_path, &path))
            })
            .map(|tab| tab.id)
        {
            self.active = Some(id);
            self.pending_focus = Some(id);
            self.pending_scroll = None;
            if self.find.open {
                self.recompute_find();
            }
            return;
        }

        match document::read_file(&path) {
            Ok(loaded) => {
                self.next_id += 1;
                let id = self.next_id;
                self.tabs.push(Tab::from_loaded(id, path.clone(), loaded));
                self.active = Some(id);
                self.pending_focus = Some(id);
                self.pending_scroll = None;
                self.config.add_recent(&path);
                if self.find.open {
                    self.recompute_find();
                }
                self.persist_config();
            }
            Err(error) => {
                let missing = !path.exists();
                if create_if_missing && missing {
                    self.next_id += 1;
                    let id = self.next_id;
                    self.tabs.push(Tab::empty_with_path(id, path));
                    self.active = Some(id);
                    self.pending_focus = Some(id);
                    self.pending_scroll = None;
                    self.persist_config();
                } else if report_errors {
                    self.show_error(
                        format!("Gagal membuka {}", path.display()),
                        error.to_string(),
                    );
                }
            }
        }
    }

    fn show_error(&mut self, title: impl Into<String>, message: impl Into<String>) {
        self.dialog = Some(Dialog::Error {
            title: title.into(),
            message: message.into(),
        });
    }

    fn persist_config(&mut self) {
        self.config.session.open_files = self
            .tabs
            .iter()
            .filter_map(|tab| tab.path.clone())
            .collect();
        self.config.session.active = self.active_tab().and_then(|tab| tab.path.clone());
        if let Some(path) = self.config_path.as_ref()
            && let Err(error) = config::save_to(&self.config, path)
        {
            eprintln!("rustpad: gagal menyimpan config: {error}");
        }
    }

    fn close_tab(&mut self, id: TabId) {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return;
        };
        let was_active = self.active == Some(id);
        self.tabs.remove(index);

        if was_active {
            self.active = if self.tabs.is_empty() {
                None
            } else {
                let next_index = index.saturating_sub(1).min(self.tabs.len() - 1);
                Some(self.tabs[next_index].id)
            };
            self.pending_focus = self.active;
            self.pending_scroll = None;
        }

        if self.tabs.is_empty() {
            self.find.close();
        } else if self.find.open {
            self.recompute_find();
        }
        self.persist_config();
    }

    fn request_close_tab(&mut self, id: TabId) {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return;
        };
        if tab.modified {
            self.dialog = Some(Dialog::CloseUnsaved {
                tab: id,
                name: tab.title(),
            });
        } else {
            self.close_tab(id);
        }
    }

    fn save_tab(&mut self, id: TabId) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if self.tabs[index].path.is_none() {
            self.start_save_as_dialog(id, SaveKind::Plain);
            return false;
        }
        self.write_tab(index)
    }

    fn write_tab(&mut self, index: usize) -> bool {
        let Some(path) = self.tabs[index].path.clone() else {
            return false;
        };
        let result = document::write_file(
            &path,
            &self.tabs[index].text,
            self.tabs[index].eol,
            self.tabs[index].bom,
        );
        match result {
            Ok(()) => {
                self.tabs[index].modified = false;
                self.config.add_recent(&path);
                self.persist_config();
                true
            }
            Err(error) => {
                self.show_error(
                    format!("Gagal menyimpan {}", path.display()),
                    error.to_string(),
                );
                false
            }
        }
    }

    fn save_tab_as_path(&mut self, id: TabId, path: PathBuf) -> bool {
        let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
            return false;
        };
        if path.is_dir() {
            self.show_error(
                "Tidak bisa menyimpan",
                format!("{} adalah folder.", path.display()),
            );
            return false;
        }
        if let Some(other_id) = other_tab_with_path(&self.tabs, id, &path) {
            let other_title = self
                .tabs
                .iter()
                .find(|tab| tab.id == other_id)
                .map(|tab| tab.title())
                .unwrap_or_default();
            self.show_error(
                "Path sudah terbuka",
                format!(
                    "{} sudah dibuka di tab \"{other_title}\". Tutup tab itu dulu.",
                    path.display()
                ),
            );
            return false;
        }
        let result = document::write_file(
            &path,
            &self.tabs[index].text,
            self.tabs[index].eol,
            self.tabs[index].bom,
        );
        match result {
            Ok(()) => {
                self.tabs[index].set_path(path.clone());
                self.tabs[index].modified = false;
                self.config.add_recent(&path);
                self.persist_config();
                true
            }
            Err(error) => {
                self.show_error(
                    format!("Gagal menyimpan {}", path.display()),
                    error.to_string(),
                );
                false
            }
        }
    }

    fn start_save_as_dialog(&mut self, id: TabId, kind: SaveKind) -> bool {
        let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
            return false;
        };
        let suggested = tab
            .path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "untitled.txt".to_owned());
        let purpose = match kind {
            SaveKind::Plain => DialogPurpose::SaveAs(id),
            SaveKind::CloseAfter => DialogPurpose::SaveAsForClose(id),
            SaveKind::QuitAfter => DialogPurpose::SaveAsForQuit(id),
        };
        self.fd.start_save_as(purpose, &suggested)
    }

    fn begin_quit(&mut self, ctx: &Context, from_os: bool) {
        if self.closing || self.quit.active {
            if from_os {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            return;
        }
        if self.fd.is_pending() {
            if from_os {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
            }
            self.show_error(
                "Dialog file masih terbuka",
                "Selesaikan atau batalkan dialog file sebelum keluar.",
            );
            return;
        }
        let dirty = self.tabs.iter().filter(|tab| tab.modified).count();
        if dirty == 0 {
            self.finalize_quit(ctx);
            return;
        }
        if from_os {
            ctx.send_viewport_cmd(ViewportCommand::CancelClose);
        }
        self.quit.active = true;
        self.quit.saving = false;
        self.dialog = Some(Dialog::Quit { dirty });
    }

    fn finalize_quit(&mut self, ctx: &Context) {
        self.persist_config();
        self.closing = true;
        self.quit.active = false;
        self.quit.saving = false;
        self.quit.queue.clear();
        self.dialog = None;
        ctx.send_viewport_cmd(ViewportCommand::Close);
    }

    fn process_quit_queue(&mut self, ctx: &Context) {
        if self.closing || !self.quit.active || !self.quit.saving || self.fd.is_pending() {
            return;
        }
        loop {
            let Some(&id) = self.quit.queue.front() else {
                let dirty: Vec<TabId> = self
                    .tabs
                    .iter()
                    .filter(|tab| tab.modified)
                    .map(|tab| tab.id)
                    .collect();
                if dirty.is_empty() {
                    self.finalize_quit(ctx);
                    return;
                }
                self.quit.queue.extend(dirty);
                continue;
            };
            let Some(index) = self.tabs.iter().position(|tab| tab.id == id) else {
                self.quit.queue.pop_front();
                continue;
            };
            if !self.tabs[index].modified {
                self.quit.queue.pop_front();
                continue;
            }
            if self.tabs[index].path.is_none() {
                if !self.start_save_as_dialog(id, SaveKind::QuitAfter) {
                    return;
                }
                self.quit.queue.pop_front();
                return;
            }
            if !self.write_tab(index) {
                self.quit.active = false;
                self.quit.saving = false;
                self.quit.queue.clear();
                return;
            }
            self.quit.queue.pop_front();
        }
    }

    fn handle_dialog_result(
        &mut self,
        ctx: &Context,
        purpose: DialogPurpose,
        result: DialogResult,
    ) {
        match (purpose, result) {
            (DialogPurpose::Open, DialogResult::Opened(paths)) => {
                for path in paths {
                    self.open_path(path, false, true);
                }
            }
            (DialogPurpose::SaveAs(id), DialogResult::SavePath(Some(path))) => {
                self.save_tab_as_path(id, path);
            }
            (DialogPurpose::SaveAsForClose(id), DialogResult::SavePath(Some(path))) => {
                if self.save_tab_as_path(id, path) {
                    self.close_tab(id);
                }
            }
            (DialogPurpose::SaveAsForQuit(id), DialogResult::SavePath(Some(path))) => {
                if self.save_tab_as_path(id, path) {
                    self.process_quit_queue(ctx);
                } else {
                    self.quit.active = false;
                    self.quit.saving = false;
                    self.quit.queue.clear();
                }
            }
            (DialogPurpose::SaveAsForQuit(_), DialogResult::SavePath(None)) => {
                self.quit.active = false;
                self.quit.saving = false;
                self.quit.queue.clear();
            }
            (_, DialogResult::SavePath(None))
            | (_, DialogResult::Opened(_))
            | (DialogPurpose::Open, DialogResult::SavePath(Some(_))) => {}
        }
    }

    fn handle_dialog_action(&mut self, ctx: &Context, action: DialogAction) {
        match action {
            DialogAction::Dismiss => self.dialog = None,
            DialogAction::QuitSaveAll => {
                self.dialog = None;
                self.quit.active = true;
                self.quit.saving = true;
                self.quit.queue = self
                    .tabs
                    .iter()
                    .filter(|tab| tab.modified)
                    .map(|tab| tab.id)
                    .collect();
                self.process_quit_queue(ctx);
            }
            DialogAction::QuitDiscard => {
                self.dialog = None;
                self.finalize_quit(ctx);
            }
            DialogAction::QuitCancel => {
                self.dialog = None;
                self.quit.active = false;
                self.quit.saving = false;
                self.quit.queue.clear();
            }
            DialogAction::SaveTab(id) => {
                self.dialog = None;
                let has_path = self
                    .tabs
                    .iter()
                    .find(|tab| tab.id == id)
                    .is_some_and(|tab| tab.path.is_some());
                if has_path {
                    if self.save_tab(id) {
                        self.close_tab(id);
                    }
                } else {
                    self.start_save_as_dialog(id, SaveKind::CloseAfter);
                }
            }
            DialogAction::DiscardTab(id) => {
                self.dialog = None;
                self.close_tab(id);
            }
            DialogAction::CancelCloseTab => self.dialog = None,
            DialogAction::OpenPath(text) => {
                self.dialog = None;
                let path = expand_tilde(&text);
                if path.is_file() {
                    self.open_path(path, false, true);
                } else {
                    self.show_error("File tidak ditemukan", path.display().to_string());
                }
            }
            DialogAction::GoToLine(line) => {
                self.dialog = None;
                self.jump_to_line(ctx, line);
            }
        }
    }

    fn open_find(&mut self, replace: bool) {
        if self.active.is_none() {
            return;
        }
        self.find.open_find(replace);
        self.recompute_find();
    }

    fn recompute_find(&mut self) {
        if let Some(index) = self.active_index() {
            self.find.recompute(&self.tabs[index].text);
        } else {
            self.find.matches.clear();
            self.find.current = None;
        }
    }

    fn find_next(&mut self, ctx: &Context, backwards: bool) {
        let Some(id) = self.active else {
            return;
        };
        if self.find.query.is_empty() {
            return;
        }
        if !self.find.open {
            self.find.open = true;
            self.recompute_find();
        }
        let selected = if backwards {
            self.find.select_prev()
        } else {
            self.find.select_next()
        };
        let Some(index) = selected else {
            return;
        };
        let (start, end) = {
            let Some(tab) = self.tabs.iter().find(|tab| tab.id == id) else {
                return;
            };
            let Some(range) = self.find.matches.get(index) else {
                return;
            };
            (
                text_index::byte_to_char(&tab.text, range.start),
                text_index::byte_to_char(&tab.text, range.end),
            )
        };
        self.set_selection(ctx, id, start, end);
        self.pending_scroll = Some(start);
    }

    fn set_selection(&mut self, ctx: &Context, tab_id: TabId, start: usize, end: usize) {
        let id = editor::editor_id(tab_id);
        if let Some(mut state) = TextEdit::load_state(ctx, id) {
            state.cursor.set_char_range(Some(CCursorRange::two(
                CCursor::new(start),
                CCursor::new(end),
            )));
            TextEdit::store_state(ctx, id, state);
        }
        self.deferred_focus = Some(tab_id);
    }

    fn clear_undoer(&self, ctx: &Context, tab_id: TabId) {
        let id = editor::editor_id(tab_id);
        if let Some(mut state) = TextEdit::load_state(ctx, id) {
            state.clear_undoer();
            TextEdit::store_state(ctx, id, state);
        }
    }

    fn replace_current(&mut self, ctx: &Context) {
        let Some(index) = self.active_index() else {
            return;
        };
        if self.find.current.is_none() && !self.find.matches.is_empty() {
            self.find.current = Some(0);
        }
        if self.find.current_range().is_none() {
            return;
        }
        let new_byte = self.find.replace_current(&mut self.tabs[index].text);
        let Some(new_byte) = new_byte else {
            return;
        };
        let id = self.tabs[index].id;
        self.tabs[index].mark_changed();
        self.clear_undoer(ctx, id);

        if let Some(range) = self.find.current_range() {
            let (start, end) = (
                text_index::byte_to_char(&self.tabs[index].text, range.start),
                text_index::byte_to_char(&self.tabs[index].text, range.end),
            );
            self.set_selection(ctx, id, start, end);
            self.pending_scroll = Some(start);
        } else {
            let char_pos = text_index::byte_to_char(&self.tabs[index].text, new_byte);
            self.set_selection(ctx, id, char_pos, char_pos);
            self.pending_scroll = Some(char_pos);
        }
    }

    fn replace_all(&mut self, ctx: &Context) {
        let Some(index) = self.active_index() else {
            return;
        };
        let count = self.find.replace_all(&mut self.tabs[index].text);
        if count == 0 {
            return;
        }
        let id = self.tabs[index].id;
        self.tabs[index].mark_changed();
        self.clear_undoer(ctx, id);
        self.pending_scroll = None;
    }

    fn handle_action(&mut self, ctx: &Context, action: Action) {
        match action {
            Action::NewTab => self.new_tab(),
            Action::Open => {
                self.fd.start_open();
            }
            Action::OpenPathDialog => {
                self.dialog = Some(Dialog::OpenPath {
                    path: String::new(),
                    focus: true,
                });
            }
            Action::OpenRecent(path) => {
                if path.is_file() {
                    self.open_path(path, false, true);
                } else {
                    self.config.recent_files.retain(|recent| recent != &path);
                    self.persist_config();
                    self.show_error(
                        "File tidak ditemukan",
                        format!("{} sudah tidak ada.", path.display()),
                    );
                }
            }
            Action::Save => {
                if let Some(id) = self.active {
                    self.save_tab(id);
                }
            }
            Action::SaveAs => {
                if let Some(id) = self.active {
                    self.start_save_as_dialog(id, SaveKind::Plain);
                }
            }
            Action::CloseActiveTab => {
                if let Some(id) = self.active {
                    self.request_close_tab(id);
                }
            }
            Action::CloseTab(id) => self.request_close_tab(id),
            Action::ActivateTab(id) => self.activate_tab(id),
            Action::NextTab => self.cycle_tab(1),
            Action::PrevTab => self.cycle_tab(-1),
            Action::GoToLineDialog => {
                if let Some(tab) = self.active_tab() {
                    let line_count = tab.line_index.line_count();
                    self.dialog = Some(Dialog::GoToLine {
                        line: String::new(),
                        focus: true,
                        line_count,
                    });
                }
            }
            Action::Quit => self.begin_quit(ctx, false),
            Action::Find => self.open_find(false),
            Action::Replace => self.open_find(true),
            Action::FindNext => {
                if self.find.query.is_empty() {
                    self.open_find(false);
                } else {
                    self.find_next(ctx, false);
                }
            }
            Action::FindPrev => {
                if self.find.query.is_empty() {
                    self.open_find(false);
                } else {
                    self.find_next(ctx, true);
                }
            }
            Action::CloseFind => {
                self.find.close();
                self.pending_focus = self.active;
            }
            Action::ReplaceCurrent => self.replace_current(ctx),
            Action::ReplaceAll => self.replace_all(ctx),
            Action::SetWrap(value) => {
                self.config.word_wrap = value;
                self.persist_config();
            }
            Action::SetTheme(theme) => {
                self.config.theme = theme;
                apply_theme(ctx, theme);
                self.persist_config();
            }
            Action::CycleTheme => {
                let next = match self.config.theme {
                    ThemePref::System => ThemePref::Light,
                    ThemePref::Light => ThemePref::Dark,
                    ThemePref::Dark => ThemePref::System,
                };
                self.config.theme = next;
                apply_theme(ctx, next);
                self.persist_config();
            }
            Action::ZoomIn => {
                let zoom = (ctx.zoom_factor() + 0.1).min(3.0);
                ctx.set_zoom_factor(zoom);
            }
            Action::ZoomOut => {
                let zoom = (ctx.zoom_factor() - 0.1).max(0.5);
                ctx.set_zoom_factor(zoom);
            }
            Action::ZoomReset => ctx.set_zoom_factor(1.0),
            Action::About => self.dialog = Some(Dialog::About),
        }
    }

    fn shortcut_actions(&self, ctx: &Context) -> Vec<Action> {
        if self.dialog.is_some() || self.quit.active || self.closing {
            return Vec::new();
        }
        let mut actions = Vec::new();
        ctx.input_mut(|input| {
            if input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::S) {
                actions.push(Action::SaveAs);
            } else if input.consume_key(Modifiers::CTRL, Key::S) {
                actions.push(Action::Save);
            }
            if input.consume_key(Modifiers::CTRL, Key::N) {
                actions.push(Action::NewTab);
            }
            if input.consume_key(Modifiers::CTRL, Key::O) {
                actions.push(Action::Open);
            }
            if input.consume_key(Modifiers::CTRL, Key::W) {
                actions.push(Action::CloseActiveTab);
            }
            if input.consume_key(Modifiers::CTRL, Key::Q) {
                actions.push(Action::Quit);
            }
            if input.consume_key(Modifiers::CTRL, Key::F) {
                actions.push(Action::Find);
            }
            if input.consume_key(Modifiers::CTRL, Key::H) {
                actions.push(Action::Replace);
            }
            if input.consume_key(Modifiers::CTRL | Modifiers::SHIFT, Key::Tab) {
                actions.push(Action::PrevTab);
            } else if input.consume_key(Modifiers::CTRL, Key::Tab) {
                actions.push(Action::NextTab);
            }
            if input.consume_key(Modifiers::CTRL, Key::PageUp) {
                actions.push(Action::PrevTab);
            } else if input.consume_key(Modifiers::CTRL, Key::PageDown) {
                actions.push(Action::NextTab);
            }
            if input.consume_key(Modifiers::CTRL, Key::G) {
                actions.push(Action::GoToLineDialog);
            }
            if input.consume_key(Modifiers::SHIFT, Key::F3) {
                actions.push(Action::FindPrev);
            } else if input.consume_key(Modifiers::NONE, Key::F3) {
                actions.push(Action::FindNext);
            }
            if input.consume_key(Modifiers::NONE, Key::Escape) && self.find.open {
                actions.push(Action::CloseFind);
            }
        });
        actions
    }

    fn find_bar(&mut self, ui: &mut Ui) -> Vec<Action> {
        let mut actions = Vec::new();
        let mut refresh = false;
        let p = theme::palette(ui.visuals().dark_mode);

        ui.spacing_mut().item_spacing.y = 6.0;
        ui.horizontal(|ui| {
            ui.add_sized(
                [52.0, 22.0],
                egui::Label::new(RichText::new("Find").size(11.0).color(p.muted)),
            );
            let response = ui.add(
                TextEdit::singleline(&mut self.find.query)
                    .desired_width(230.0)
                    .hint_text("Cari teks")
                    .background_color(p.surface)
                    .text_color(p.text),
            );
            if self.find.focus_query {
                response.request_focus();
                self.find.focus_query = false;
            }
            if response.changed() {
                refresh = true;
            }
            let enter_target = response.has_focus() || response.lost_focus();
            let (enter_pressed, shift_enter) = if enter_target {
                ui.input_mut(|input| {
                    let shift_enter = input.consume_key(Modifiers::SHIFT, Key::Enter);
                    let plain_enter = input.consume_key(Modifiers::NONE, Key::Enter);
                    (plain_enter || shift_enter, shift_enter)
                })
            } else {
                (false, false)
            };
            if enter_pressed {
                if shift_enter {
                    actions.push(Action::FindPrev);
                } else {
                    actions.push(Action::FindNext);
                }
            }
            if ui.add(Button::new("Prev").small()).clicked() {
                actions.push(Action::FindPrev);
            }
            if ui.add(Button::new("Next").small()).clicked() {
                actions.push(Action::FindNext);
            }

            let mut case_sensitive = self.find.case_sensitive;
            if ui
                .checkbox(&mut case_sensitive, "Aa")
                .on_hover_text("Case sensitive")
                .changed()
            {
                self.find.case_sensitive = case_sensitive;
                refresh = true;
            }

            if !self.find.query.is_empty() {
                if self.find.matches.is_empty() {
                    theme::chip(ui, "No results");
                } else {
                    let count = format!(
                        "{} / {}",
                        self.find.current.map(|index| index + 1).unwrap_or(0),
                        self.find.matches.len()
                    );
                    theme::chip_accent(ui, &count);
                }
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if theme::icon_button(ui, theme::Icon::Close, "Close (Esc)", true).clicked() {
                    actions.push(Action::CloseFind);
                }
            });
        });

        if self.find.replace_open {
            ui.horizontal(|ui| {
                ui.add_sized(
                    [52.0, 22.0],
                    egui::Label::new(RichText::new("Replace").size(11.0).color(p.muted)),
                );
                ui.add(
                    TextEdit::singleline(&mut self.find.replacement)
                        .desired_width(230.0)
                        .hint_text("Teks pengganti")
                        .background_color(p.surface)
                        .text_color(p.text),
                );
                if ui.add(Button::new("Replace").small()).clicked() {
                    actions.push(Action::ReplaceCurrent);
                }
                if theme::accent_button_small(ui, "Replace All").clicked() {
                    actions.push(Action::ReplaceAll);
                }
            });
        }

        if refresh {
            self.recompute_find();
        }
        actions
    }

    fn update_title(&mut self, ctx: &Context) {
        let title = match self.active_tab() {
            Some(tab) => format!(
                "{}{} — Rustpad",
                tab.title(),
                if tab.modified { "*" } else { "" }
            ),
            None => "Rustpad".to_owned(),
        };
        if title != self.window_title {
            ctx.send_viewport_cmd(ViewportCommand::Title(title.clone()));
            self.window_title = title;
        }
    }
}

impl eframe::App for RustpadApp {
    fn ui(&mut self, ui: &mut Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        if ctx.input(|input| input.viewport().close_requested()) && !self.closing {
            if self.quit.active {
                ctx.send_viewport_cmd(ViewportCommand::CancelClose);
                return;
            }
            self.begin_quit(&ctx, true);
            ctx.request_repaint();
            return;
        }

        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input
                .raw
                .dropped_files
                .iter()
                .map(|file| file.path().to_path_buf())
                .collect()
        });
        for path in dropped {
            self.open_path(path, false, true);
        }

        if let Some((purpose, result)) = self.fd.poll() {
            self.handle_dialog_result(&ctx, purpose, result);
        }
        if self.quit.active {
            self.process_quit_queue(&ctx);
        }

        if let Some(id) = self.deferred_focus.take()
            && self.active == Some(id)
        {
            ctx.memory_mut(|memory| memory.request_focus(editor::editor_id(id)));
        }

        let mut actions = self.shortcut_actions(&ctx);
        let recent_files = self.config.recent_files.clone();
        let theme = self.config.theme;
        let word_wrap = self.config.word_wrap;
        let has_tab = self.active.is_some();

        let palette = theme::palette(ui.visuals().dark_mode);
        Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(theme::margin(12, 12, 6, 6)),
            )
            .show(ui, |ui| {
                ui.scope(|ui| {
                    let v = ui.visuals_mut();
                    v.widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                    v.widgets.inactive.bg_fill = Color32::TRANSPARENT;
                    v.widgets.inactive.bg_stroke = Stroke::NONE;
                    v.widgets.hovered.weak_bg_fill = palette.surface_hover;
                    v.widgets.hovered.bg_fill = palette.surface_hover;
                    v.widgets.hovered.bg_stroke = Stroke::NONE;
                    v.widgets.hovered.fg_stroke = Stroke::new(1.0, palette.text);
                    ui.spacing_mut().button_padding = egui::vec2(8.0, 4.0);
                    egui::MenuBar::new().ui(ui, |ui| {
                        let _ = theme::brand_mark(ui, 21.0);
                        ui.add_space(2.0);
                        ui.label(RichText::new("Rustpad").size(15.5).color(palette.text));
                        ui.add_space(10.0);
                        ui.menu_button("File", |ui| {
                            if ui
                                .add(Button::new("New Tab").shortcut_text("Ctrl+N"))
                                .clicked()
                            {
                                actions.push(Action::NewTab);
                                ui.close();
                            }
                            if ui
                                .add(Button::new("Open…").shortcut_text("Ctrl+O"))
                                .clicked()
                            {
                                actions.push(Action::Open);
                                ui.close();
                            }
                            if ui.button("Open Path…").clicked() {
                                actions.push(Action::OpenPathDialog);
                                ui.close();
                            }
                            ui.menu_button("Recent Files", |ui| {
                                if recent_files.is_empty() {
                                    ui.label(RichText::new("Belum ada file terakhir").weak());
                                }
                                for path in &recent_files {
                                    if ui.button(path.display().to_string()).clicked() {
                                        actions.push(Action::OpenRecent(path.clone()));
                                        ui.close();
                                    }
                                }
                            });
                            ui.separator();
                            if ui
                                .add_enabled(has_tab, Button::new("Save").shortcut_text("Ctrl+S"))
                                .clicked()
                            {
                                actions.push(Action::Save);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Save As…").shortcut_text("Ctrl+Shift+S"),
                                )
                                .clicked()
                            {
                                actions.push(Action::SaveAs);
                                ui.close();
                            }
                            ui.separator();
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Close Tab").shortcut_text("Ctrl+W"),
                                )
                                .clicked()
                            {
                                actions.push(Action::CloseActiveTab);
                                ui.close();
                            }
                            if ui
                                .add(Button::new("Quit").shortcut_text("Ctrl+Q"))
                                .clicked()
                            {
                                actions.push(Action::Quit);
                                ui.close();
                            }
                        });

                        ui.menu_button("Edit", |ui| {
                            if ui
                                .add_enabled(has_tab, Button::new("Find…").shortcut_text("Ctrl+F"))
                                .clicked()
                            {
                                actions.push(Action::Find);
                                ui.close();
                            }
                            if ui
                                .add_enabled(has_tab, Button::new("Find Next").shortcut_text("F3"))
                                .clicked()
                            {
                                actions.push(Action::FindNext);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Find Previous").shortcut_text("Shift+F3"),
                                )
                                .clicked()
                            {
                                actions.push(Action::FindPrev);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Replace…").shortcut_text("Ctrl+H"),
                                )
                                .clicked()
                            {
                                actions.push(Action::Replace);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Go to Line…").shortcut_text("Ctrl+G"),
                                )
                                .clicked()
                            {
                                actions.push(Action::GoToLineDialog);
                                ui.close();
                            }
                        });

                        ui.menu_button("View", |ui| {
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Next Tab").shortcut_text("Ctrl+Tab"),
                                )
                                .clicked()
                            {
                                actions.push(Action::NextTab);
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    has_tab,
                                    Button::new("Previous Tab").shortcut_text("Ctrl+Shift+Tab"),
                                )
                                .clicked()
                            {
                                actions.push(Action::PrevTab);
                                ui.close();
                            }
                            ui.separator();
                            let mut wrap = word_wrap;
                            if ui.checkbox(&mut wrap, "Word wrap").changed() {
                                actions.push(Action::SetWrap(wrap));
                            }
                            ui.menu_button("Theme", |ui| {
                                if ui.radio(theme == ThemePref::System, "System").clicked() {
                                    actions.push(Action::SetTheme(ThemePref::System));
                                    ui.close();
                                }
                                if ui.radio(theme == ThemePref::Light, "Light").clicked() {
                                    actions.push(Action::SetTheme(ThemePref::Light));
                                    ui.close();
                                }
                                if ui.radio(theme == ThemePref::Dark, "Dark").clicked() {
                                    actions.push(Action::SetTheme(ThemePref::Dark));
                                    ui.close();
                                }
                            });
                            ui.separator();
                            if ui.button("Zoom In").clicked() {
                                actions.push(Action::ZoomIn);
                                ui.close();
                            }
                            if ui.button("Zoom Out").clicked() {
                                actions.push(Action::ZoomOut);
                                ui.close();
                            }
                            if ui.button("Reset Zoom").clicked() {
                                actions.push(Action::ZoomReset);
                                ui.close();
                            }
                        });

                        ui.menu_button("Help", |ui| {
                            if ui.button("About Rustpad").clicked() {
                                actions.push(Action::About);
                                ui.close();
                            }
                        });

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let theme_label = match theme {
                                ThemePref::System => "System",
                                ThemePref::Light => "Light",
                                ThemePref::Dark => "Dark",
                            };
                            if theme::icon_button(
                                ui,
                                theme::Icon::Theme,
                                &format!("Theme: {theme_label} (click to cycle)"),
                                true,
                            )
                            .clicked()
                            {
                                actions.push(Action::CycleTheme);
                            }
                            if theme::icon_button(ui, theme::Icon::Save, "Save (Ctrl+S)", has_tab)
                                .clicked()
                            {
                                actions.push(Action::Save);
                            }
                            if theme::icon_button(ui, theme::Icon::Search, "Find (Ctrl+F)", has_tab)
                                .clicked()
                            {
                                actions.push(Action::Find);
                            }
                        });
                    });
                });
            });

        if !self.tabs.is_empty() {
            Panel::top("tab_bar")
                .frame(
                    egui::Frame::new()
                        .fill(palette.panel_alt)
                        .inner_margin(theme::margin(10, 10, 5, 5)),
                )
                .show(ui, |ui| {
                    actions.extend(tab_bar(ui, &self.tabs, self.active));
                });
        }

        if self.find.open && self.active.is_some() {
            Panel::top("find_bar")
                .frame(
                    egui::Frame::new()
                        .fill(palette.panel_alt)
                        .inner_margin(theme::margin(12, 12, 8, 8)),
                )
                .show(ui, |ui| {
                    actions.extend(self.find_bar(ui));
                });
        }

        if !actions.is_empty() {
            for action in actions {
                self.handle_action(&ctx, action);
            }
            ctx.request_repaint();
        }

        if let Some(id) = self.pending_focus {
            if self.active == Some(id) {
                ctx.memory_mut(|memory| memory.request_focus(editor::editor_id(id)));
            }
            self.pending_focus = None;
        }

        let info = self.info;
        let tab_meta = self.active_tab().map(|tab| {
            (
                tab.lang.label,
                tab.eol.label(),
                tab.path.clone(),
                document::highlighting_enabled(&tab.text),
                document::is_large(&tab.text),
            )
        });
        Panel::bottom("status_bar")
            .frame(
                egui::Frame::new()
                    .fill(palette.panel)
                    .inner_margin(theme::margin(12, 12, 5, 5)),
            )
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if let Some((language, eol, path, highlighting, large)) = &tab_meta {
                        ui.label(
                            RichText::new(format!("Ln {}, Col {}", info.line, info.col))
                                .size(11.5)
                                .color(palette.text),
                        );
                        if info.selection_chars > 0 {
                            theme::chip(ui, &format!("{} selected", info.selection_chars));
                        }
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            let full_path = path
                                .as_ref()
                                .map(|path| path.display().to_string())
                                .unwrap_or_else(|| "untitled".to_owned());
                            ui.add(
                                egui::Label::new(
                                    RichText::new(full_path.clone()).size(11.0).weak(),
                                )
                                .truncate(),
                            )
                            .on_hover_text(full_path);
                            if !highlighting {
                                theme::chip(ui, "highlight off");
                            }
                            if *large {
                                theme::chip(ui, "large file");
                            }
                            theme::chip(ui, "UTF-8");
                            theme::chip(ui, eol);
                            theme::chip_accent(ui, language);
                        });
                    } else {
                        ui.label(
                            RichText::new("Tidak ada file terbuka")
                                .size(11.5)
                                .color(palette.muted),
                        );
                    }
                });
            });

        let mut welcome_actions = Vec::new();
        CentralPanel::default().show(ui, |ui| {
            if let Some(index) = self.active_index() {
                let scroll_to = self.pending_scroll.take();
                let highlight = &self.highlight;
                let wrap = self.config.word_wrap;
                let find_matches: &[std::ops::Range<usize>] = if self.find.open {
                    self.find.matches.as_slice()
                } else {
                    &[]
                };
                let active_match = if self.find.open {
                    self.find.current_range()
                } else {
                    None
                };
                let tab = &mut self.tabs[index];
                let editor_info = editor::show(
                    ui,
                    tab,
                    wrap,
                    highlight,
                    scroll_to,
                    find_matches,
                    active_match.as_ref(),
                );
                self.info = editor_info;
                if editor_info.changed && self.find.open {
                    self.recompute_find();
                }
            } else {
                let rect = ui.max_rect();
                let painter = ui.painter();
                painter.circle_filled(
                    egui::pos2(rect.right() - 40.0, rect.top() + 20.0),
                    260.0,
                    theme::tint(palette.accent, if palette.dark { 12 } else { 16 }),
                );
                painter.circle_filled(
                    egui::pos2(rect.left() + 30.0, rect.bottom() - 20.0),
                    320.0,
                    theme::tint(palette.text, if palette.dark { 8 } else { 5 }),
                );

                ui.vertical_centered(|ui| {
                    ui.add_space((ui.available_height() * 0.10).max(24.0));
                    let _ = theme::logo_mark(ui, 74.0);
                    ui.add_space(14.0);
                    ui.label(RichText::new("Rustpad").size(40.0).color(palette.text));
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new(
                            "Editor teks ringan untuk Linux — cepat, tenang, tanpa distraksi.",
                        )
                        .size(14.0)
                        .color(palette.muted),
                    );
                    ui.add_space(22.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(272.0, 36.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            if theme::accent_button(ui, "New file", 132.0).clicked() {
                                welcome_actions.push(Action::NewTab);
                            }
                            ui.add_space(8.0);
                            if theme::secondary_button(ui, "Open file…", 132.0).clicked() {
                                welcome_actions.push(Action::Open);
                            }
                        },
                    );
                    ui.add_space(12.0);
                    ui.allocate_ui_with_layout(
                        egui::vec2(250.0, 18.0),
                        Layout::left_to_right(Align::Center),
                        |ui| {
                            ui.label(RichText::new("Ctrl+N").size(10.5).color(palette.muted));
                            ui.label(RichText::new("new").size(10.5).weak());
                            ui.add_space(9.0);
                            ui.label(RichText::new("Ctrl+O").size(10.5).color(palette.muted));
                            ui.label(RichText::new("open").size(10.5).weak());
                            ui.add_space(9.0);
                            ui.label(RichText::new("Ctrl+F").size(10.5).color(palette.muted));
                            ui.label(RichText::new("find").size(10.5).weak());
                        },
                    );
                    ui.add_space(28.0);

                    if !recent_files.is_empty() {
                        ui.allocate_ui_with_layout(
                            egui::vec2(460.0, 0.0),
                            Layout::top_down(Align::LEFT),
                            |ui| {
                                ui.label(
                                    RichText::new("RECENT FILES")
                                        .size(10.0)
                                        .color(palette.muted),
                                );
                                ui.add_space(6.0);
                                egui::Frame::new()
                                    .fill(palette.panel)
                                    .corner_radius(theme::corner(12))
                                    .stroke(Stroke::new(1.0, palette.border))
                                    .inner_margin(theme::margin(6, 6, 6, 6))
                                    .show(ui, |ui| {
                                        ui.set_width(448.0);
                                        for path in recent_files.iter().take(5) {
                                            let name = path
                                                .file_name()
                                                .map(|name| name.to_string_lossy().into_owned())
                                                .unwrap_or_else(|| path.display().to_string());
                                            let parent = path
                                                .parent()
                                                .map(|parent| parent.display().to_string())
                                                .unwrap_or_default();
                                            if theme::list_row(ui, &name, &parent).clicked() {
                                                welcome_actions
                                                    .push(Action::OpenRecent(path.clone()));
                                            }
                                        }
                                    });
                            },
                        );
                    }

                    ui.add_space(22.0);
                    ui.label(
                        RichText::new(format!("Rustpad {}", env!("CARGO_PKG_VERSION")))
                            .size(10.0)
                            .color(palette.muted),
                    );
                });
            }
        });

        if !welcome_actions.is_empty() {
            for action in welcome_actions {
                self.handle_action(&ctx, action);
            }
            ctx.request_repaint();
        }

        let dialog_action = if let Some(dialog) = self.dialog.as_mut() {
            dialogs::show(&ctx, dialog)
        } else {
            None
        };
        if let Some(action) = dialog_action {
            self.handle_dialog_action(&ctx, action);
            ctx.request_repaint();
        }

        self.update_title(&ctx);
        if self.fd.is_pending() {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
    }

    fn persist_egui_memory(&self) -> bool {
        false
    }
}

fn tab_bar(ui: &mut Ui, tabs: &[Tab], active: Option<TabId>) -> Vec<Action> {
    let mut actions = Vec::new();
    ScrollArea::horizontal().max_height(32.0).show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            for tab in tabs {
                ui.push_id(tab.id, |ui| {
                    let tooltip = tab
                        .path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| tab.title());
                    match theme::tab_pill(
                        ui,
                        &tab.title(),
                        active == Some(tab.id),
                        tab.modified,
                        &tooltip,
                    ) {
                        Some(theme::TabAction::Activate) => {
                            actions.push(Action::ActivateTab(tab.id));
                        }
                        Some(theme::TabAction::Close) => actions.push(Action::CloseTab(tab.id)),
                        None => {}
                    }
                });
            }
            if theme::plus_button(ui) {
                actions.push(Action::NewTab);
            }
        });
    });
    actions
}

fn apply_theme(ctx: &Context, preference: ThemePref) {
    ctx.set_theme(match preference {
        ThemePref::System => ThemePreference::System,
        ThemePref::Light => ThemePreference::Light,
        ThemePref::Dark => ThemePreference::Dark,
    });
}

fn expand_tilde(input: &str) -> PathBuf {
    if let Some(rest) = input.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(input)
}

fn paths_equal(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }
    match (left.canonicalize(), right.canonicalize()) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn absolute_path(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(&path))
        .unwrap_or(path)
}

fn other_tab_with_path(tabs: &[Tab], except: TabId, path: &Path) -> Option<TabId> {
    tabs.iter()
        .find(|tab| {
            tab.id != except
                && tab
                    .path
                    .as_deref()
                    .is_some_and(|open_path| paths_equal(open_path, path))
        })
        .map(|tab| tab.id)
}

fn cycle_index(current: usize, len: usize, delta: isize) -> Option<usize> {
    if len == 0 {
        return None;
    }
    Some((current as isize + delta).rem_euclid(len as isize) as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_absolute_paths_unchanged() {
        let path = PathBuf::from("/tmp/contoh.txt");
        assert_eq!(absolute_path(path.clone()), path);
    }

    #[test]
    fn resolves_relative_paths_against_the_working_directory() {
        let path = absolute_path(PathBuf::from("contoh.txt"));
        assert!(path.is_absolute());
        assert_eq!(path, std::env::current_dir().unwrap().join("contoh.txt"));
    }

    #[test]
    fn detects_another_tab_with_the_same_path() {
        let tabs = vec![
            Tab::empty_with_path(1, PathBuf::from("/tmp/a.txt")),
            Tab::empty_with_path(2, PathBuf::from("/tmp/b.txt")),
        ];
        assert_eq!(
            other_tab_with_path(&tabs, 1, Path::new("/tmp/b.txt")),
            Some(2)
        );
        assert_eq!(other_tab_with_path(&tabs, 2, Path::new("/tmp/b.txt")), None);
        assert_eq!(other_tab_with_path(&tabs, 1, Path::new("/tmp/c.txt")), None);
    }

    #[test]
    fn cycles_tab_indices_with_wrap() {
        assert_eq!(cycle_index(0, 3, 1), Some(1));
        assert_eq!(cycle_index(2, 3, 1), Some(0));
        assert_eq!(cycle_index(0, 3, -1), Some(2));
        assert_eq!(cycle_index(1, 3, -1), Some(0));
        assert_eq!(cycle_index(0, 1, 1), Some(0));
        assert_eq!(cycle_index(0, 0, 1), None);
    }
}
