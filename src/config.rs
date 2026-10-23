use serde::{Deserialize, Serialize};
use std::io;
use std::path::{Path, PathBuf};

pub const RECENT_LIMIT: usize = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemePref {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Session {
    pub open_files: Vec<PathBuf>,
    pub active: Option<PathBuf>,
    pub cursors: Vec<CursorEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CursorEntry {
    pub path: PathBuf,
    pub char: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub theme: ThemePref,
    pub word_wrap: bool,
    pub recent_files: Vec<PathBuf>,
    pub session: Session,
}

impl Config {
    pub fn add_recent(&mut self, path: &Path) {
        let path = path.to_path_buf();
        self.recent_files.retain(|existing| existing != &path);
        self.recent_files.insert(0, path);
        self.recent_files.truncate(RECENT_LIMIT);
    }

    /// Membuang recent files yang sudah tidak ada di disk.
    pub fn prune_recent(&mut self) {
        self.recent_files.retain(|path| path.is_file());
    }
}

pub fn config_path_from(
    xdg_config_home: Option<PathBuf>,
    home: Option<PathBuf>,
) -> Option<PathBuf> {
    let base = xdg_config_home.or_else(|| home.map(|home| home.join(".config")))?;
    Some(base.join("rustpad").join("config.toml"))
}

pub fn config_path() -> Option<PathBuf> {
    let xdg = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty());
    config_path_from(xdg, home)
}

pub fn load_from(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|contents| toml::from_str(&contents).ok())
        .unwrap_or_default()
}

pub fn save_to(config: &Config, path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let contents = toml::to_string_pretty(config).map_err(io::Error::other)?;
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, contents)?;
    std::fs::rename(&tmp_path, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rustpad-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn resolves_xdg_config_home_first() {
        let path =
            config_path_from(Some(PathBuf::from("/xdg")), Some(PathBuf::from("/home/me"))).unwrap();
        assert_eq!(path, PathBuf::from("/xdg/rustpad/config.toml"));
    }

    #[test]
    fn falls_back_to_home_dot_config() {
        let path = config_path_from(None, Some(PathBuf::from("/home/me"))).unwrap();
        assert_eq!(path, PathBuf::from("/home/me/.config/rustpad/config.toml"));
    }

    #[test]
    fn returns_none_without_any_base() {
        assert_eq!(config_path_from(None, None), None);
    }

    #[test]
    fn round_trips_config_through_disk() {
        let dir = temp_dir("roundtrip");
        let path = dir.join("rustpad").join("config.toml");
        let mut config = Config {
            theme: ThemePref::Dark,
            word_wrap: true,
            ..Config::default()
        };
        config.add_recent(Path::new("/tmp/satu.txt"));
        config.session = Session {
            open_files: vec![PathBuf::from("/tmp/satu.txt")],
            active: Some(PathBuf::from("/tmp/satu.txt")),
            cursors: vec![CursorEntry {
                path: PathBuf::from("/tmp/satu.txt"),
                char: 42,
            }],
        };

        save_to(&config, &path).unwrap();
        let loaded = load_from(&path);
        assert_eq!(loaded, config);
    }

    #[test]
    fn missing_or_corrupt_file_falls_back_to_defaults() {
        let dir = temp_dir("corrupt");
        let missing = dir.join("tidak-ada.toml");
        assert_eq!(load_from(&missing), Config::default());

        let corrupt = dir.join("rusak.toml");
        fs::write(&corrupt, "ini = bukan [toml").unwrap();
        assert_eq!(load_from(&corrupt), Config::default());
    }

    #[test]
    fn recent_files_are_deduped_and_capped() {
        let mut config = Config::default();
        for i in 0..12 {
            config.add_recent(Path::new(&format!("/tmp/file-{i}.txt")));
        }
        assert_eq!(config.recent_files.len(), RECENT_LIMIT);
        assert_eq!(config.recent_files[0], PathBuf::from("/tmp/file-11.txt"));

        config.add_recent(Path::new("/tmp/file-5.txt"));
        assert_eq!(config.recent_files.len(), RECENT_LIMIT);
        assert_eq!(config.recent_files[0], PathBuf::from("/tmp/file-5.txt"));
        let count = config
            .recent_files
            .iter()
            .filter(|p| p.as_path() == Path::new("/tmp/file-5.txt"))
            .count();
        assert_eq!(count, 1);
    }

    #[test]
    fn prune_removes_missing_files() {
        let dir = temp_dir("prune");
        let exists = dir.join("ada.txt");
        fs::write(&exists, "isi").unwrap();
        let missing = dir.join("hilang.txt");

        let mut config = Config {
            recent_files: vec![missing, exists.clone()],
            ..Config::default()
        };
        config.prune_recent();
        assert_eq!(config.recent_files, vec![exists]);
    }

    #[test]
    fn legacy_session_without_cursors_uses_defaults() {
        let dir = temp_dir("legacy");
        let path = dir.join("config.toml");
        fs::write(
            &path,
            "theme = \"dark\"\n\n[session]\nopen_files = [\"/tmp/satu.txt\"]\nactive = \"/tmp/satu.txt\"\n",
        )
        .unwrap();

        let loaded = load_from(&path);
        assert!(loaded.session.cursors.is_empty());
        assert_eq!(
            loaded.session.active.as_deref(),
            Some(Path::new("/tmp/satu.txt"))
        );
    }
}
