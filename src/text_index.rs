/// Peta baris berdasarkan indeks karakter (bukan byte) untuk teks UTF-8.
#[derive(Clone, Debug, Default)]
pub struct LineIndex {
    starts: Vec<usize>,
    char_count: usize,
}

impl LineIndex {
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        let mut char_count = 0;
        for (idx, ch) in text.chars().enumerate() {
            char_count += 1;
            if ch == '\n' {
                starts.push(idx + 1);
            }
        }
        Self { starts, char_count }
    }

    pub fn line_count(&self) -> usize {
        self.starts.len()
    }

    pub fn line_col(&self, char_index: usize) -> (usize, usize) {
        let char_index = char_index.min(self.char_count);
        let line_idx = match self.starts.binary_search(&char_index) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        (line_idx + 1, char_index - self.starts[line_idx] + 1)
    }

    /// Indeks karakter di awal baris (1-based). None jika di luar jangkauan.
    pub fn line_start_char(&self, line: usize) -> Option<usize> {
        if line == 0 {
            return None;
        }
        self.starts.get(line - 1).copied()
    }
}

pub fn byte_to_char(text: &str, byte_index: usize) -> usize {
    let byte_index = byte_index.min(text.len());
    text.char_indices()
        .take_while(|(idx, _)| *idx < byte_index)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_text_is_one_line() {
        let idx = LineIndex::new("");
        assert_eq!(idx.line_count(), 1);
        assert_eq!(idx.line_col(0), (1, 1));
    }

    #[test]
    fn tracks_line_and_column_after_newlines() {
        let idx = LineIndex::new("abc\nde\n");
        assert_eq!(idx.line_count(), 3);
        assert_eq!(idx.line_col(0), (1, 1));
        assert_eq!(idx.line_col(3), (1, 4));
        assert_eq!(idx.line_col(4), (2, 1));
        assert_eq!(idx.line_col(7), (3, 1));
    }

    #[test]
    fn clamps_index_past_end() {
        let idx = LineIndex::new("ab");
        assert_eq!(idx.line_col(99), (1, 3));
    }

    #[test]
    fn counts_unicode_with_char_indices() {
        let idx = LineIndex::new("aé\nb");
        assert_eq!(idx.line_col(2), (1, 3));
        assert_eq!(idx.line_col(3), (2, 1));
    }

    #[test]
    fn converts_byte_offsets_to_char_offsets() {
        let text = "aé🦀b";
        assert_eq!(byte_to_char(text, 0), 0);
        assert_eq!(byte_to_char(text, 1), 1);
        assert_eq!(byte_to_char(text, 3), 2);
        assert_eq!(byte_to_char(text, 7), 3);
    }

    #[test]
    fn resolves_line_start_char_indices() {
        let idx = LineIndex::new("aé\nb\n");
        assert_eq!(idx.line_start_char(1), Some(0));
        assert_eq!(idx.line_start_char(2), Some(3));
        assert_eq!(idx.line_start_char(3), Some(5));
        assert_eq!(idx.line_start_char(0), None);
        assert_eq!(idx.line_start_char(4), None);
    }
}
