use regex::RegexBuilder;
use std::ops::Range;

#[derive(Clone, Debug, Default)]
pub struct FindState {
    pub open: bool,
    pub replace_open: bool,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub matches: Vec<Range<usize>>,
    pub current: Option<usize>,
    pub focus_query: bool,
}

pub fn find_matches(text: &str, query: &str, case_sensitive: bool) -> Vec<Range<usize>> {
    if query.is_empty() {
        return Vec::new();
    }
    let pattern = regex::escape(query);
    let Ok(regex) = RegexBuilder::new(&pattern)
        .case_insensitive(!case_sensitive)
        .build()
    else {
        return Vec::new();
    };
    regex.find_iter(text).map(|m| m.range()).collect()
}

impl FindState {
    pub fn recompute(&mut self, text: &str) {
        self.matches = find_matches(text, &self.query, self.case_sensitive);
        if self.matches.is_empty() {
            self.current = None;
        } else if let Some(index) = self.current {
            self.current = Some(index.min(self.matches.len() - 1));
        }
    }

    /// Pilih match berikutnya, wrap-around. `None` jika tidak ada match.
    pub fn select_next(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let index = match self.current {
            Some(index) => (index + 1) % self.matches.len(),
            None => 0,
        };
        self.current = Some(index);
        Some(index)
    }

    /// Pilih match sebelumnya, wrap-around. `None` jika tidak ada match.
    pub fn select_prev(&mut self) -> Option<usize> {
        if self.matches.is_empty() {
            self.current = None;
            return None;
        }
        let index = match self.current {
            Some(0) => self.matches.len() - 1,
            Some(index) => index - 1,
            None => self.matches.len() - 1,
        };
        self.current = Some(index);
        Some(index)
    }

    pub fn current_range(&self) -> Option<Range<usize>> {
        self.current
            .and_then(|index| self.matches.get(index))
            .cloned()
    }

    /// Mengganti match aktif. Mengembalikan posisi byte akhir hasil replace.
    pub fn replace_current(&mut self, text: &mut String) -> Option<usize> {
        self.recompute(text);
        let range = self.current_range()?;
        text.replace_range(range.clone(), &self.replacement);
        let new_position = range.start + self.replacement.len();
        self.recompute(text);
        Some(new_position)
    }

    /// Mengganti semua match, mengembalikan jumlah penggantian.
    pub fn replace_all(&mut self, text: &mut String) -> usize {
        self.recompute(text);
        let count = self.matches.len();
        if count == 0 {
            return 0;
        }
        for range in self.matches.iter().rev() {
            text.replace_range(range.clone(), &self.replacement);
        }
        self.recompute(text);
        self.current = None;
        count
    }

    pub fn open_find(&mut self, replace: bool) {
        self.open = true;
        self.replace_open = replace;
        self.focus_query = true;
    }

    pub fn close(&mut self) {
        self.open = false;
        self.replace_open = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_literal_matches() {
        assert_eq!(
            find_matches("halo dunia halo", "halo", true),
            vec![0..4, 11..15]
        );
    }

    #[test]
    fn treats_query_as_literal_text() {
        assert_eq!(find_matches("a.b axb", "a.b", true), vec![0..3]);
        assert_eq!(find_matches("harga $5", "$5", true), vec![6..8]);
    }

    #[test]
    fn supports_case_insensitive_and_unicode() {
        assert_eq!(
            find_matches("Halo halo HALO", "halo", false),
            vec![0..4, 5..9, 10..14]
        );
        assert_eq!(find_matches("café", "é", true), vec![3..5]);
        assert_eq!(find_matches("É", "é", false), vec![0..2]);
    }

    #[test]
    fn empty_query_has_no_matches() {
        assert!(find_matches("apa saja", "", true).is_empty());
    }

    #[test]
    fn matches_do_not_overlap() {
        assert_eq!(find_matches("aaa", "aa", true), vec![0..2]);
    }

    fn state(text: &str, query: &str) -> FindState {
        let mut state = FindState {
            query: query.into(),
            case_sensitive: true,
            ..FindState::default()
        };
        state.recompute(text);
        state
    }

    #[test]
    fn navigation_wraps_around() {
        let mut state = state("satu dua satu", "satu");
        assert_eq!(state.select_next(), Some(0));
        assert_eq!(state.select_next(), Some(1));
        assert_eq!(state.select_next(), Some(0));
        assert_eq!(state.select_prev(), Some(1));
        assert_eq!(state.select_prev(), Some(0));
        assert_eq!(state.select_prev(), Some(1));
    }

    #[test]
    fn navigation_without_matches_returns_none() {
        let mut state = state("tidak ada", "zzz");
        assert_eq!(state.select_next(), None);
        assert_eq!(state.select_prev(), None);
        assert_eq!(state.current_range(), None);
    }

    #[test]
    fn recompute_clamps_current_index() {
        let mut state = state("a a a", "a");
        state.current = Some(2);
        state.recompute("a");
        assert_eq!(state.current, Some(0));
        state.recompute("zzz");
        assert_eq!(state.current, None);
    }

    #[test]
    fn replace_current_replaces_only_active_match() {
        let mut state = state("foo bar foo", "foo");
        state.replacement = "baz".into();
        state.select_next();

        let mut text = "foo bar foo".to_string();
        let new_pos = state.replace_current(&mut text).unwrap();
        assert_eq!(new_pos, 3);
        assert_eq!(text, "baz bar foo");
        assert_eq!(state.matches, vec![8..11]);
        assert_eq!(state.current, Some(0));
    }

    #[test]
    fn replace_all_treats_replacement_literally() {
        let mut text = "a.b a.b".to_string();
        let mut state = state(&text, "a.b");
        state.replacement = "$1 🦀".into();
        let count = state.replace_all(&mut text);
        assert_eq!(count, 2);
        assert_eq!(text, "$1 🦀 $1 🦀");
        assert!(state.matches.is_empty());
        assert_eq!(state.current, None);
    }

    #[test]
    fn replace_all_without_matches_is_noop() {
        let mut text = "tidak ada".to_string();
        let mut state = state(&text, "zzz");
        state.replacement = "x".into();
        assert_eq!(state.replace_all(&mut text), 0);
        assert_eq!(text, "tidak ada");
    }

    #[test]
    fn replace_current_recomputes_when_ranges_are_stale() {
        let mut state = state("aaaa", "aa");
        state.replacement = "baru".into();
        state.current = Some(1);

        let mut text = "aa".to_string();
        let position = state.replace_current(&mut text).unwrap();
        assert_eq!(text, "baru");
        assert_eq!(position, 4);
        assert!(state.matches.is_empty());
    }

    #[test]
    fn replace_all_ignores_ranges_from_another_buffer() {
        let mut state = state("lama lama", "lama");
        state.replacement = "baru".into();
        let mut text = "teks lain".to_string();

        assert_eq!(state.replace_all(&mut text), 0);
        assert_eq!(text, "teks lain");
        assert!(state.matches.is_empty());
    }

    #[test]
    fn open_and_close_manage_visibility() {
        let mut state = FindState::default();
        state.open_find(true);
        assert!(state.open);
        assert!(state.replace_open);
        assert!(state.focus_query);
        state.close();
        assert!(!state.open);
        assert!(!state.replace_open);
    }
}
