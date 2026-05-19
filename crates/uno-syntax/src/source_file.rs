use crate::diagnostic::{Diagnostic, DiagnosticBag, Severity};
use crate::span::Span;
use std::fmt::Write;
use std::io::IsTerminal;

pub struct SourceFile {
    source: String,
    line_starts: Vec<usize>,
    path: Option<String>,
}

impl SourceFile {
    pub fn new(source: String) -> Self {
        let line_starts = std::iter::once(0)
            .chain(source.match_indices('\n').map(|(i, _)| i + 1))
            .collect();
        SourceFile {
            source,
            line_starts,
            path: None,
        }
    }

    pub fn with_path(source: String, path: impl Into<String>) -> Self {
        let mut sf = Self::new(source);
        sf.path = Some(path.into());
        sf
    }

    pub fn path(&self) -> Option<&str> {
        self.path.as_deref()
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
        self.format_diagnostic(&Diagnostic::error(
            crate::diagnostic::ErrorCode::E001,
            message,
            span,
        ))
    }

    pub fn format_diagnostic(&self, diag: &Diagnostic) -> String {
        let (line, col) = self.line_col(diag.span.start);
        let text = self.line_text(line);
        let (_color, red, yellow, cyan, bold, reset) = if std::io::stderr().is_terminal() {
            ("", "\x1b[1;31m", "\x1b[1;33m", "\x1b[1;36m", "\x1b[1m", "\x1b[0m")
        } else {
            ("", "", "", "", "", "")
        };

        let (severity_color, severity_label) = match diag.severity {
            Severity::Error => (red, "error"),
            Severity::Warning => (yellow, "warning"),
            Severity::Note => (cyan, "note"),
        };

        let location = if let Some(ref path) = self.path {
            format!("{path}:{line}:{col}")
        } else {
            format!("{line}:{col}")
        };

        let underline = if diag.span.len() > 1 && diag.span.len() <= text.len().saturating_sub(col - 1) {
            " ".repeat(col - 1) + &"^".repeat(diag.span.len().min(40))
        } else {
            " ".repeat(col - 1) + "^"
        };

        let mut out = String::new();
        let _ = writeln!(
            out,
            "{severity_color}{bold}{severity_label}[{code}]{reset}{bold}: {msg}{reset}",
            code = diag.code.as_str(),
            msg = diag.message
        );
        let _ = writeln!(out, "{bold}  --> {location}{reset}");
        let _ = writeln!(out, "{bold}   |{reset}");
        let _ = writeln!(out, "{bold}{line:>3} |{reset} {text}");
        let _ = writeln!(out, "{bold}   |{reset} {severity_color}{underline}{reset}");

        if let Some(ref hint) = diag.hint {
            let _ = writeln!(
                out,
                "{bold}   ={reset} {cyan}help:{reset} {hint}"
            );
        }

        out
    }

    pub fn format_diagnostics(&self, bag: &DiagnosticBag) -> String {
        let mut out = String::new();
        for diag in &bag.diagnostics {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&self.format_diagnostic(diag));
        }

        let errors = bag.error_count();
        let warnings = bag.warning_count();
        if !bag.is_empty() {
            let _ = writeln!(out);
            let _ = write!(out, "{} error(s)", errors);
            if warnings > 0 {
                let _ = write!(out, ", {} warning(s)", warnings);
            }
            let _ = writeln!(out);
        }

        out
    }

    pub fn format_parse_error(&self, err: &crate::error::ParseError) -> String {
        self.format_diagnostic(&err.to_diagnostic())
    }
}

impl std::fmt::Debug for SourceFile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SourceFile")
            .field("path", &self.path)
            .field("lines", &self.line_starts.len())
            .finish()
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
        assert!(msg.contains("error"), "got: {msg}");
        assert!(msg.contains("2:5"), "got: {msg}");
        assert!(msg.contains("return 0;"), "got: {msg}");
        assert!(msg.contains("^"), "got: {msg}");
        assert!(msg.contains("expected semicolon"), "got: {msg}");
    }
}
