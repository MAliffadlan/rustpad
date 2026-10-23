use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, TryRecvError};

use crate::document::TabId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DialogPurpose {
    Open,
    SaveAs(TabId),
    SaveAsForClose(TabId),
    SaveAsForQuit(TabId),
}

pub enum DialogResult {
    Opened(Vec<PathBuf>),
    SavePath(Option<PathBuf>),
}

struct Pending {
    purpose: DialogPurpose,
    rx: Receiver<DialogResult>,
}

#[derive(Default)]
pub struct FileDialogService {
    pending: Option<Pending>,
}

impl FileDialogService {
    pub fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub fn start_open(&mut self) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let files = rfd::FileDialog::new()
                .add_filter(
                    "Text",
                    &[
                        "txt", "md", "rs", "py", "js", "ts", "json", "toml", "yaml", "yml", "html",
                        "css", "sh", "c", "cpp", "go", "java", "sql", "xml", "ini",
                    ],
                )
                .pick_files()
                .unwrap_or_default();
            let _ = tx.send(DialogResult::Opened(files));
        });
        self.pending = Some(Pending {
            purpose: DialogPurpose::Open,
            rx,
        });
        true
    }

    pub fn start_save_as(&mut self, purpose: DialogPurpose, suggested_name: &str) -> bool {
        if self.pending.is_some() {
            return false;
        }
        let (tx, rx) = mpsc::channel();
        let suggested_name = suggested_name.to_owned();
        std::thread::spawn(move || {
            let path = rfd::FileDialog::new()
                .set_file_name(&suggested_name)
                .save_file();
            let _ = tx.send(DialogResult::SavePath(path));
        });
        self.pending = Some(Pending { purpose, rx });
        true
    }

    pub fn poll(&mut self) -> Option<(DialogPurpose, DialogResult)> {
        let pending = self.pending.as_ref()?;
        match pending.rx.try_recv() {
            Ok(result) => {
                let purpose = pending.purpose;
                self.pending = None;
                Some((purpose, result))
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                let purpose = pending.purpose;
                self.pending = None;
                let result = match purpose {
                    DialogPurpose::Open => DialogResult::Opened(Vec::new()),
                    _ => DialogResult::SavePath(None),
                };
                Some((purpose, result))
            }
        }
    }
}
