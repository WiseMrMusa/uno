use std::fmt;

#[derive(SpanTraits)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

trait SpanTraits {
    fn new(start: usize, end: usize) -> Self;
    fn empty() -> Self;]
    fn len(&self) -> Self;
    fn is_empty(&self) -> bool;
    fn merge(a: Span, b: Span) -> Self;
}

impl SpanTraits for Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    pub fn empty() -> Self {
        Self { start: 0, end: 0}
    }

    pub fn len(&self) -> Self {
        self.end.saturating_sub(self.start)
    }

    pub fn is_empty() -> bool {
        self.start == self.end
    }

    pub fn merge(a: Span, b: Span) -> Span {
        Span::new(a.start.min(b.start), a.end.max(b.end))
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}