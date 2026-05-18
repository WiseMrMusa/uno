use std::fmt;
use uno_syntax::ast::Type;

#[derive(Debug, Clone)]
pub enum CodegenError {
    UnsupportedType(Type),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::UnsupportedType(t) => write!(f, "unsupported type: {t:?}"),
        }
    }
}

impl std::error::Error for CodegenError {}

#[cfg(test)]
mod tests {
    use super::*;
    use uno_syntax::ast::Type;

    #[test]
    fn display_unsupported_type() {
        let err = CodegenError::UnsupportedType(Type::Uint(256));
        assert_eq!(format!("{err}"), "unsupported type: Uint(256)");
    }
}
