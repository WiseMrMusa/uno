use crate::diagnostic::{Diagnostic, DiagnosticBag, ErrorCode};
use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
    pub code: ErrorCode,
    pub hint: Option<String>,
}

impl ParseError {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            code: ErrorCode::E001,
            hint: None,
        }
    }

    pub fn with_code(code: ErrorCode, message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
            code,
            hint: None,
        }
    }

    pub fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    pub fn to_diagnostic(&self) -> Diagnostic {
        let mut d = Diagnostic::error(self.code, &self.message, self.span);
        if let Some(ref h) = self.hint {
            d = d.with_hint(h.clone());
        }
        d
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "error[{}]: {}", self.code.as_str(), self.message)
    }
}

impl std::error::Error for ParseError {}

impl From<ParseError> for DiagnosticBag {
    fn from(err: ParseError) -> Self {
        let mut bag = DiagnosticBag::new();
        bag.error(err.code, err.message, err.span);
        bag
    }
}
