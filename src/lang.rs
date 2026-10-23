use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Language {
    /// Token yang dikenali syntect (nama atau ekstensi).
    pub token: &'static str,
    /// Label yang ditampilkan di status bar.
    pub label: &'static str,
}

pub const PLAIN: Language = Language {
    token: "txt",
    label: "Plain Text",
};

pub fn detect(path: Option<&Path>) -> Language {
    let Some(path) = path else {
        return PLAIN;
    };

    if path.file_name().and_then(|n| n.to_str()) == Some("Cargo.lock") {
        return Language {
            token: "toml",
            label: "TOML",
        };
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    match ext.as_str() {
        "rs" => Language {
            token: "rs",
            label: "Rust",
        },
        "py" => Language {
            token: "py",
            label: "Python",
        },
        "js" | "mjs" | "cjs" | "jsx" => Language {
            token: "js",
            label: "JavaScript",
        },
        "ts" | "tsx" => Language {
            token: "ts",
            label: "TypeScript",
        },
        "json" | "jsonc" => Language {
            token: "json",
            label: "JSON",
        },
        "toml" => Language {
            token: "toml",
            label: "TOML",
        },
        "yaml" | "yml" => Language {
            token: "yaml",
            label: "YAML",
        },
        "md" | "markdown" => Language {
            token: "md",
            label: "Markdown",
        },
        "html" | "htm" => Language {
            token: "html",
            label: "HTML",
        },
        "css" => Language {
            token: "css",
            label: "CSS",
        },
        "scss" => Language {
            token: "scss",
            label: "SCSS",
        },
        "sh" | "bash" | "zsh" => Language {
            token: "sh",
            label: "Shell",
        },
        "c" | "h" => Language {
            token: "c",
            label: "C",
        },
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => Language {
            token: "cpp",
            label: "C++",
        },
        "go" => Language {
            token: "go",
            label: "Go",
        },
        "java" => Language {
            token: "java",
            label: "Java",
        },
        "sql" => Language {
            token: "sql",
            label: "SQL",
        },
        "xml" => Language {
            token: "xml",
            label: "XML",
        },
        "ini" => Language {
            token: "ini",
            label: "INI",
        },
        _ => PLAIN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn lang(path: &str) -> Language {
        detect(Some(Path::new(path)))
    }

    #[test]
    fn detects_common_extensions() {
        assert_eq!(lang("main.rs").label, "Rust");
        assert_eq!(lang("main.rs").token, "rs");
        assert_eq!(lang("config.toml").label, "TOML");
        assert_eq!(lang("app.ts").label, "TypeScript");
        assert_eq!(lang("README.md").label, "Markdown");
        assert_eq!(lang("script.py").label, "Python");
        assert_eq!(lang("data.json").label, "JSON");
    }

    #[test]
    fn extension_matching_is_case_insensitive() {
        assert_eq!(lang("MAIN.RS").label, "Rust");
    }

    #[test]
    fn cargo_lock_is_toml() {
        assert_eq!(lang("/tmp/Cargo.lock").label, "TOML");
    }

    #[test]
    fn unknown_or_missing_paths_are_plain_text() {
        assert_eq!(detect(None), PLAIN);
        assert_eq!(lang("file.unknownext"), PLAIN);
        assert_eq!(lang("no_extension"), PLAIN);
    }
}
