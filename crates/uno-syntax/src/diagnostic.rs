use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Severity {
    Error,
    Warning,
    Note,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorCode {
    E001, // syntax error
    E002, // type mismatch
    E003, // undefined variable
    E004, // undefined function
    E005, // lexer error
    E006, // import error
    E007, // lowering error
    E008, // codegen error
}

impl ErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ErrorCode::E001 => "E001",
            ErrorCode::E002 => "E002",
            ErrorCode::E003 => "E003",
            ErrorCode::E004 => "E004",
            ErrorCode::E005 => "E005",
            ErrorCode::E006 => "E006",
            ErrorCode::E007 => "E007",
            ErrorCode::E008 => "E008",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            ErrorCode::E001 => "syntax error",
            ErrorCode::E002 => "type mismatch",
            ErrorCode::E003 => "undefined variable",
            ErrorCode::E004 => "undefined function",
            ErrorCode::E005 => "lexer error",
            ErrorCode::E006 => "import error",
            ErrorCode::E007 => "IR lowering error",
            ErrorCode::E008 => "codegen error",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    pub code: ErrorCode,
    pub message: String,
    pub span: Span,
    pub hint: Option<String>,
}

impl Diagnostic {
    pub fn error(code: ErrorCode, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Error,
            code,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn warning(code: ErrorCode, message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            code,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn note(message: impl Into<String>, span: Span) -> Self {
        Diagnostic {
            severity: Severity::Note,
            code: ErrorCode::E001,
            message: message.into(),
            span,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Note => "note",
        };
        write!(f, "{prefix}[{}]: {}", self.code.as_str(), self.message)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Default)]
pub struct DiagnosticBag {
    pub diagnostics: Vec<Diagnostic>,
    error_count: usize,
    warning_count: usize,
}

impl DiagnosticBag {
    pub fn new() -> Self {
        DiagnosticBag {
            diagnostics: Vec::new(),
            error_count: 0,
            warning_count: 0,
        }
    }

    pub fn error(
        &mut self,
        code: ErrorCode,
        message: impl Into<String>,
        span: Span,
    ) -> &mut Diagnostic {
        self.error_count += 1;
        let diag = Diagnostic::error(code, message, span);
        self.diagnostics.push(diag);
        self.diagnostics.last_mut().unwrap()
    }

    pub fn warning(
        &mut self,
        code: ErrorCode,
        message: impl Into<String>,
        span: Span,
    ) -> &mut Diagnostic {
        self.warning_count += 1;
        let diag = Diagnostic::warning(code, message, span);
        self.diagnostics.push(diag);
        self.diagnostics.last_mut().unwrap()
    }

    pub fn has_errors(&self) -> bool {
        self.error_count > 0
    }

    pub fn error_count(&self) -> usize {
        self.error_count
    }

    pub fn warning_count(&self) -> usize {
        self.warning_count
    }

    pub fn is_empty(&self) -> bool {
        self.diagnostics.is_empty()
    }

    pub fn extend(&mut self, other: DiagnosticBag) {
        self.error_count += other.error_count;
        self.warning_count += other.warning_count;
        self.diagnostics.extend(other.diagnostics);
    }

    pub fn take_diagnostics(&mut self) -> Vec<Diagnostic> {
        self.error_count = 0;
        self.warning_count = 0;
        std::mem::take(&mut self.diagnostics)
    }

    pub fn merge(&mut self, other: &mut DiagnosticBag) {
        self.error_count += other.error_count;
        self.warning_count += other.warning_count;
        self.diagnostics.append(&mut other.diagnostics);
        other.error_count = 0;
        other.warning_count = 0;
    }
}
