use crate::span::Span;

pub struct SourceFile {
    source: String,
    line_starts: Vec<usize>,
}

impl SourceFile {
    pub fn new(source: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        SourceFile { source, line_starts }
    }

    pub fn line_col(&self, offset: usize) -> (usize, usize) {
        let offset = offset.min(self.source.len());
        match self.line_starts.binary_search(&offset) {
            Ok(line) => (line + 1, 1),
            Err(line) => {
                let line = line.saturating_sub(1);
                let col = offset - self.line_starts[line] + 1;
                (line + 1, col)
            }
        }
    }

    pub fn line_text(&self, line: usize) -> &str {
        let line = line.max(1);
        let idx = line - 1;
        if idx >= self.line_starts.len() {
            return "";
        }
        let start = self.line_starts[idx];
        let end = self.source[start..]
            .find('\n')
            .map(|i| start + i)
            .unwrap_or(self.source.len());
        &self.source[start..end]
    }

    pub fn format_error(&self, span: Span, message: &str) -> String {
        let (line, col) = self.line_col(span.start);
        let text = self.line_text(line);
        let underline = if span.len() > 1 && span.len() <= text.len().saturating_sub(col - 1) {
            " ".repeat(col - 1) + &"^".repeat(span.len().min(40))
        } else {
            " ".repeat(col - 1) + "^"
        };
        format!(
            "error --> {line}:{col}\n  |\n{line} | {text}\n  | {underline}\n  | {message}\n"
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::Span;

    #[test]
    fn line_col_simple() {
        let sf = SourceFile::new("hello\nworld\nfoo".to_string());
        assert_eq!(sf.line_col(0), (1, 1));
        assert_eq!(sf.line_col(5), (1, 6));
        assert_eq!(sf.line_col(6), (2, 1));
        assert_eq!(sf.line_col(11), (2, 6));
        assert_eq!(sf.line_col(12), (3, 1));
        assert_eq!(sf.line_col(14), (3, 3));
    }

    #[test]
    fn line_col_empty() {
        let sf = SourceFile::new(String::new());
        assert_eq!(sf.line_col(0), (1, 1));
    }

    #[test]
    fn line_text_returns_correct_line() {
        let sf = SourceFile::new("aaa\nbbb\nccc".to_string());
        assert_eq!(sf.line_text(1), "aaa");
        assert_eq!(sf.line_text(2), "bbb");
        assert_eq!(sf.line_text(3), "ccc");
    }

    #[test]
    fn format_error_shows_context() {
        let sf = SourceFile::new("fn main() -> u32 {\n    return 0;\n}".to_string());
        let span = Span::new(23, 24);
        let msg = sf.format_error(span, "expected semicolon");
        assert!(msg.contains("error --> 2:5"), "got: {msg}");
        assert!(msg.contains("return 0;"), "got: {msg}");
        assert!(msg.contains("^"), "got: {msg}");
        assert!(msg.contains("expected semicolon"), "got: {msg}");
    }
}
