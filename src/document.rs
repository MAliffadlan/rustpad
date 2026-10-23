use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::lang::{self, Language};
use crate::text_index::LineIndex;

pub const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
pub const HIGHLIGHT_LIMIT: usize = 256 * 1024;
pub const LARGE_FILE_WARN: usize = 1024 * 1024;

const BOM: [u8; 3] = [0xEF, 0xBB, 0xBF];
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

pub type TabId = u64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
}

impl Eol {
    pub fn label(self) -> &'static str {
        match self {
            Self::Lf => "LF",
            Self::Crlf => "CRLF",
        }
    }
}

#[derive(Debug)]
pub enum LoadError {
    TooLarge,
    NotUtf8,
    Binary,
    NotRegularFile,
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(f, "File terlalu besar (maksimal 10 MiB)."),
            Self::NotUtf8 => write!(f, "File bukan teks UTF-8 yang valid."),
            Self::Binary => write!(f, "File terlihat seperti data biner."),
            Self::NotRegularFile => write!(f, "Bukan file teks biasa."),
            Self::Io(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for LoadError {}

pub struct LoadedText {
    pub text: String,
    pub eol: Eol,
    pub bom: bool,
}

/// Men-decode byte menjadi teks, menormalkan EOL ke `\n`, dan mempertahankan info BOM/EOL.
pub fn decode_bytes(bytes: &[u8]) -> Result<LoadedText, LoadError> {
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(LoadError::TooLarge);
    }
    if bytes.contains(&0) {
        return Err(LoadError::Binary);
    }
    let (bom, body) = if let Some(body) = bytes.strip_prefix(&BOM) {
        (true, body)
    } else {
        (false, bytes)
    };
    let raw = std::str::from_utf8(body).map_err(|_| LoadError::NotUtf8)?;
    let eol = if raw.contains("\r\n") {
        Eol::Crlf
    } else {
        Eol::Lf
    };
    let text = raw.replace("\r\n", "\n").replace('\r', "\n");
    Ok(LoadedText { text, eol, bom })
}

/// Meng-encode teks internal (`\n`) kembali ke byte dengan EOL/BOM yang dipilih.
pub fn encode_text(text: &str, eol: Eol, bom: bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() + 3);
    if bom {
        out.extend_from_slice(&BOM);
    }
    match eol {
        Eol::Lf => out.extend_from_slice(text.as_bytes()),
        Eol::Crlf => {
            let mut lines = text.split('\n');
            if let Some(first) = lines.next() {
                out.extend_from_slice(first.as_bytes());
            }
            for line in lines {
                out.extend_from_slice(b"\r\n");
                out.extend_from_slice(line.as_bytes());
            }
        }
    }
    out
}

pub fn read_file(path: &Path) -> Result<LoadedText, LoadError> {
    let metadata = fs::metadata(path).map_err(LoadError::Io)?;
    if !metadata.file_type().is_file() {
        return Err(LoadError::NotRegularFile);
    }
    if metadata.len() > MAX_FILE_SIZE {
        return Err(LoadError::TooLarge);
    }
    let file = File::open(path).map_err(LoadError::Io)?;
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_FILE_SIZE + 1)
        .read_to_end(&mut bytes)
        .map_err(LoadError::Io)?;
    if bytes.len() as u64 > MAX_FILE_SIZE {
        return Err(LoadError::TooLarge);
    }
    decode_bytes(&bytes)
}

pub fn write_file(path: &Path, text: &str, eol: Eol, bom: bool) -> io::Result<()> {
    let target = if path.exists() {
        path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
    } else {
        path.to_path_buf()
    };
    let parent = target.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target.file_name().unwrap_or_default().to_string_lossy();
    let tmp = parent.join(format!(
        ".{file_name}.rustpad-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));

    let result = (|| {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        file.write_all(&encode_text(text, eol, bom))?;
        file.sync_all()?;
        if let Ok(metadata) = fs::metadata(&target) {
            fs::set_permissions(&tmp, metadata.permissions())?;
        }
        fs::rename(&tmp, &target)
    })();

    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

pub fn highlighting_enabled(text: &str) -> bool {
    text.len() <= HIGHLIGHT_LIMIT
}

pub fn is_large(text: &str) -> bool {
    text.len() > LARGE_FILE_WARN
}

pub struct Tab {
    pub id: TabId,
    pub path: Option<PathBuf>,
    pub text: String,
    pub eol: Eol,
    pub bom: bool,
    pub lang: Language,
    pub modified: bool,
    pub line_index: LineIndex,
    pub last_cursor: usize,
    pub restore_cursor: Option<usize>,
}

impl Tab {
    pub fn untitled(id: TabId) -> Self {
        Self {
            id,
            path: None,
            text: String::new(),
            eol: Eol::Lf,
            bom: false,
            lang: lang::PLAIN,
            modified: false,
            line_index: LineIndex::new(""),
            last_cursor: 0,
            restore_cursor: None,
        }
    }

    pub fn from_loaded(id: TabId, path: PathBuf, loaded: LoadedText) -> Self {
        let lang = lang::detect(Some(&path));
        let line_index = LineIndex::new(&loaded.text);
        Self {
            id,
            path: Some(path),
            text: loaded.text,
            eol: loaded.eol,
            bom: loaded.bom,
            lang,
            modified: false,
            line_index,
            last_cursor: 0,
            restore_cursor: None,
        }
    }

    /// Buffer kosong dengan path (dipakai untuk argumen CLI yang filenya belum ada).
    pub fn empty_with_path(id: TabId, path: PathBuf) -> Self {
        let lang = lang::detect(Some(&path));
        Self {
            id,
            path: Some(path),
            text: String::new(),
            eol: Eol::Lf,
            bom: false,
            lang,
            modified: false,
            line_index: LineIndex::new(""),
            last_cursor: 0,
            restore_cursor: None,
        }
    }

    pub fn title(&self) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("Untitled-{}", self.id))
    }

    pub fn set_path(&mut self, path: PathBuf) {
        self.lang = lang::detect(Some(&path));
        self.path = Some(path);
    }

    pub fn rebuild_index(&mut self) {
        self.line_index = LineIndex::new(&self.text);
    }

    pub fn mark_changed(&mut self) {
        self.modified = true;
        self.rebuild_index();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_file(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rustpad-doc-{name}-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        dir.join("file.txt")
    }

    #[test]
    fn decodes_plain_lf_text() {
        let loaded = decode_bytes(b"halo\ndunia\n").unwrap();
        assert_eq!(loaded.text, "halo\ndunia\n");
        assert_eq!(loaded.eol, Eol::Lf);
        assert!(!loaded.bom);
    }

    #[test]
    fn normalizes_crlf_and_records_eol() {
        let loaded = decode_bytes(b"a\r\nb\r\n").unwrap();
        assert_eq!(loaded.text, "a\nb\n");
        assert_eq!(loaded.eol, Eol::Crlf);
    }

    #[test]
    fn normalizes_lone_cr() {
        let loaded = decode_bytes(b"a\rb").unwrap();
        assert_eq!(loaded.text, "a\nb");
        assert_eq!(loaded.eol, Eol::Lf);
    }

    #[test]
    fn strips_and_records_utf8_bom() {
        let mut bytes = BOM.to_vec();
        bytes.extend_from_slice("isi".as_bytes());
        let loaded = decode_bytes(&bytes).unwrap();
        assert_eq!(loaded.text, "isi");
        assert!(loaded.bom);
    }

    #[test]
    fn rejects_binary_invalid_utf8_and_oversized() {
        assert!(matches!(decode_bytes(b"abc\0def"), Err(LoadError::Binary)));
        assert!(matches!(decode_bytes(b"\xFF\xFE"), Err(LoadError::NotUtf8)));
        let huge = vec![b'a'; MAX_FILE_SIZE as usize + 1];
        assert!(matches!(decode_bytes(&huge), Err(LoadError::TooLarge)));
    }

    #[test]
    fn encodes_crlf_and_bom() {
        let bytes = encode_text("a\nb\n", Eol::Crlf, true);
        let mut expected = BOM.to_vec();
        expected.extend_from_slice(b"a\r\nb\r\n");
        assert_eq!(bytes, expected);
    }

    #[test]
    fn lf_without_bom_encodes_plain() {
        assert_eq!(encode_text("a\nb", Eol::Lf, false), b"a\nb".to_vec());
    }

    #[test]
    fn file_round_trip_preserves_eol_and_bom() {
        let path = temp_file("roundtrip");
        write_file(&path, "satu\ndua\n", Eol::Crlf, true).unwrap();
        let loaded = read_file(&path).unwrap();
        assert_eq!(loaded.text, "satu\ndua\n");
        assert_eq!(loaded.eol, Eol::Crlf);
        assert!(loaded.bom);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn rejects_special_files() {
        assert!(matches!(
            read_file(Path::new("/dev/null")),
            Err(LoadError::NotRegularFile)
        ));

        let dir = std::env::temp_dir().join(format!("rustpad-doc-special-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        assert!(matches!(read_file(&dir), Err(LoadError::NotRegularFile)));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_replaces_the_file_instead_of_truncating_it() {
        use std::os::unix::fs::MetadataExt;

        let path = temp_file("atomic");
        write_file(&path, "awal\n", Eol::Lf, false).unwrap();
        let before = fs::metadata(&path).unwrap().ino();
        write_file(&path, "baru\n", Eol::Lf, false).unwrap();
        let after = fs::metadata(&path).unwrap().ino();

        assert_ne!(before, after);
        assert_eq!(fs::read_to_string(&path).unwrap(), "baru\n");
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_through_symlink_keeps_the_link() {
        use std::os::unix::fs::symlink;

        let dir = std::env::temp_dir().join(format!("rustpad-doc-symlink-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("target.txt");
        fs::write(&target, "lama").unwrap();
        let link = dir.join("link.txt");
        symlink(&target, &link).unwrap();

        write_file(&link, "baru", Eol::Lf, false).unwrap();

        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_to_string(&target).unwrap(), "baru");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn failed_save_cleans_up_the_temporary_file() {
        let dir = std::env::temp_dir().join(format!("rustpad-doc-failsave-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let target = dir.join("folder");
        fs::create_dir(&target).unwrap();

        assert!(write_file(&target, "isi", Eol::Lf, false).is_err());
        assert!(target.is_dir());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_name().to_string_lossy().contains(".rustpad-"))
            .collect();
        assert!(leftovers.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn highlighting_only_below_limit() {
        let at_limit = "a".repeat(HIGHLIGHT_LIMIT);
        let over_limit = "a".repeat(HIGHLIGHT_LIMIT + 1);
        assert!(highlighting_enabled(&at_limit));
        assert!(!highlighting_enabled(&over_limit));
    }

    #[test]
    fn tab_titles_and_language_follow_path() {
        let mut tab = Tab::untitled(7);
        assert_eq!(tab.title(), "Untitled-7");
        assert_eq!(tab.lang, lang::PLAIN);
        assert!(!tab.modified);

        tab.set_path(PathBuf::from("/tmp/contoh.rs"));
        assert_eq!(tab.title(), "contoh.rs");
        assert_eq!(tab.lang.label, "Rust");
    }

    #[test]
    fn tab_from_loaded_builds_index() {
        let tab = Tab::from_loaded(
            1,
            PathBuf::from("/tmp/dua.toml"),
            LoadedText {
                text: "a\nb".into(),
                eol: Eol::Lf,
                bom: false,
            },
        );
        assert_eq!(tab.lang.label, "TOML");
        assert_eq!(tab.line_index.line_col(2), (2, 1));
        assert!(!tab.modified);
    }

    #[test]
    fn mark_changed_updates_index() {
        let mut tab = Tab::untitled(1);
        tab.text = "a\nb".into();
        tab.mark_changed();
        assert!(tab.modified);
        assert_eq!(tab.line_index.line_count(), 2);
    }
}
