
use uno_syntax::ast::{BinOp, Block, Expr, FnDef, Param, Program, Stmt, Type, UnOp};
use uno_syntax::error::ParseError;
use uno_syntax::span::Span;
use uno_syntax::token::{Token, TokenKind};

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn kind(&self) -> TokenKind {
        self.tokens
            .get(self.pos)
            .map(|t| t.kind.clone())
            .unwrap_or(TokenKind::Eof)
    }

    fn token(&self) -> Token {
        self.tokens.get(self.pos).cloned().unwrap_or(Token {
            kind: TokenKind::Eof,
            span: Span::empty(),
        })
    }

    fn advance(&mut self) -> Token {
        let t = self.token();
        self.pos += 1;
        t
    }

    fn check(&self, kind: &TokenKind) -> bool {
        self.kind() == *kind
    }

    fn expect(&mut self, kind: &TokenKind) -> Result<Token, ParseError> {
        if self.check(kind) {
            Ok(self.advance())
        } else {
            Err(ParseError::new(
                format!("expected {:?}, found {:?}", kind, self.kind()),
                self.token().span,
            ))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.kind() {
            TokenKind::Ident(_) => {
                if let TokenKind::Ident(name) = self.advance().kind {
                    Ok(name)
                } else {
                    unreachable!()
                }
            }
            _ => Err(ParseError::new(
                format!("expected identifier, found {:?}", self.kind()),
                self.token().span,
            )),
        }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut functions = Vec::new();
        while !self.check(&TokenKind::Eof) {
            if let TokenKind::Error(msg) = self.kind() {
                let t = self.advance();
                return Err(ParseError::new(format!("lex error: {msg}"), t.span));
            }
            functions.push(self.parse_fn_def()?);
        }
        Ok(Program { functions })
    }

    fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
        if let TokenKind::Error(msg) = self.kind() {
            let t = self.advance();
            return Err(ParseError::new(format!("lex error: {msg}"), t.span));
        }
        let start_span = self.token().span;
        let public = if self.check(&TokenKind::Pub) {
            self.advance();
            true
        } else {
            false
        };
        self.expect(&TokenKind::Fn)?;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::LParen)?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::RParen) {
            loop {
                let param_span = self.token().span;
                let param_name = self.expect_ident()?;
                self.expect(&TokenKind::Colon)?;
                let param_type = self.parse_type()?;
                params.push(Param {
                    name: param_name,
                    type_: param_type,
                    span: Span::merge(param_span, self.tokens[self.pos.saturating_sub(1)].span),
                });
                if !self.check(&TokenKind::Comma) {
                    break;
                }
                self.advance();
            }
        }
        self.expect(&TokenKind::RParen)?;
        self.expect(&TokenKind::Arrow)?;
        let return_type = self.parse_type()?;
        let body = self.parse_block()?;
        let span = Span::merge(start_span, body.span);
        Ok(FnDef {
            name,
            params,
            return_type,
            body,
            span,
            public,
        })
    }

    fn parse_block(&mut self) -> Result<Block, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::LBrace)?;

        let mut stmts = Vec::new();
        while !self.check(&TokenKind::RBrace) && !self.check(&TokenKind::Eof) {
            stmts.push(self.parse_stmt()?);
        }

        let end = self.token().span;
        self.expect(&TokenKind::RBrace)?;
        Ok(Block {
            stmts,
            span: Span::merge(start, end),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ParseError> {
        if let TokenKind::Error(msg) = self.kind() {
            let t = self.advance();
            return Err(ParseError::new(format!("lex error: {msg}"), t.span));
        }
        match self.kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::If => self.parse_if_stmt(),
            TokenKind::While => self.parse_while_stmt(),
            TokenKind::Loop => self.parse_loop_stmt(),
            TokenKind::Break => self.parse_break_stmt(),
            TokenKind::Ident(_) => {
                if self.tokens.get(self.pos + 1).map(|t| t.kind == TokenKind::Equals).unwrap_or(false) {
                    self.parse_assign_stmt()
                } else {
                    self.parse_expr_stmt()
                }
            }
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::Let)?;
        let is_mut = if self.check(&TokenKind::Mut) {
            self.advance();
            true
        } else {
            false
        };
        let name = self.expect_ident()?;
        let type_ = if self.check(&TokenKind::Colon) {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&TokenKind::Equals)?;
        let value = self.parse_expr()?;
        self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Let(
            name,
            is_mut,
            type_,
            value,
            Span::merge(start, self.tokens[self.pos.saturating_sub(1)].span),
        ))
    }

    fn parse_return_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::Return)?;
        if self.check(&TokenKind::Semicolon) {
            let semi = self.advance();
            Ok(Stmt::Return(None, Span::merge(start, semi.span)))
        } else {
            let value = self.parse_expr()?;
            let semi = self.expect(&TokenKind::Semicolon)?;
            Ok(Stmt::Return(Some(value), Span::merge(start, semi.span)))
        }
    }

    fn parse_if_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::If)?;
        let cond = self.parse_expr()?;
        let then_block = self.parse_block()?;
        let else_block = if self.check(&TokenKind::Else) {
            self.advance();
            if self.check(&TokenKind::If) {
                let else_if = self.parse_if_stmt()?;
                let block_span = expr_span_for_stmt(&else_if);
                let block = Block {
                    stmts: vec![else_if],
                    span: block_span,
                };
                Some(block)
            } else {
                Some(self.parse_block()?)
            }
        } else {
            None
        };
        let end = match &else_block {
            Some(b) => b.span,
            None => then_block.span,
        };
        Ok(Stmt::If(cond, then_block, else_block, Span::merge(start, end)))
    }

    fn parse_while_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::While)?;
        let cond = self.parse_expr()?;
        let body = self.parse_block()?;
        let body_span = body.span;
        Ok(Stmt::While(cond, body, Span::merge(start, body_span)))
    }

    fn parse_loop_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::Loop)?;
        let body = self.parse_block()?;
        let body_span = body.span;
        Ok(Stmt::Loop(body, Span::merge(start, body_span)))
    }

    fn parse_assign_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        let name = self.expect_ident()?;
        self.expect(&TokenKind::Equals)?;
        let value = self.parse_expr()?;
        let semi = self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Assign(name, value, Span::merge(start, semi.span)))
    }

    fn parse_break_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::Break)?;
        let semi = self.expect(&TokenKind::Semicolon)?;
        Ok(Stmt::Break(Span::merge(start, semi.span)))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, ParseError> {
        let expr = self.parse_expr()?;
        let span = if self.check(&TokenKind::Semicolon) {
            let semi = self.advance();
            Span::merge(expr_span(&expr), semi.span)
        } else {
            expr_span(&expr)
        };
        Ok(Stmt::Expr(expr, span))
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_or()
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_and()?;
        while self.check(&TokenKind::OrOr) {
            self.advance();
            let right = self.parse_and()?;
            let span = Span::merge(expr_span(&expr), expr_span(&right));
            expr = Expr::BinaryOp(Box::new(expr), BinOp::Or, Box::new(right), span);
        }
        Ok(expr)
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_comparison()?;
        while self.check(&TokenKind::AndAnd) {
            self.advance();
            let right = self.parse_comparison()?;
            let span = Span::merge(expr_span(&expr), expr_span(&right));
            expr = Expr::BinaryOp(Box::new(expr), BinOp::And, Box::new(right), span);
        }
        Ok(expr)
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_additive()?;
        loop {
            let op = match self.kind() {
                TokenKind::EqualsEquals => BinOp::Eq,
                TokenKind::NotEquals => BinOp::Neq,
                TokenKind::Less => BinOp::Lt,
                TokenKind::Greater => BinOp::Gt,
                TokenKind::LessEquals => BinOp::Le,
                TokenKind::GreaterEquals => BinOp::Ge,
                _ => break,
            };
            self.advance();
            let right = self.parse_additive()?;
            let span = Span::merge(expr_span(&expr), expr_span(&right));
            expr = Expr::BinaryOp(Box::new(expr), op, Box::new(right), span);
        }
        Ok(expr)
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_multiplicative()?;
        loop {
            let op = match self.kind() {
                TokenKind::Plus => BinOp::Add,
                TokenKind::Minus => BinOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_multiplicative()?;
            let span = Span::merge(expr_span(&expr), expr_span(&right));
            expr = Expr::BinaryOp(Box::new(expr), op, Box::new(right), span);
        }
        Ok(expr)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_unary()?;
        loop {
            let op = match self.kind() {
                TokenKind::Star => BinOp::Mul,
                TokenKind::Slash => BinOp::Div,
                TokenKind::Percent => BinOp::Mod,
                _ => break,
            };
            self.advance();
            let right = self.parse_unary()?;
            let span = Span::merge(expr_span(&expr), expr_span(&right));
            expr = Expr::BinaryOp(Box::new(expr), op, Box::new(right), span);
        }
        Ok(expr)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.kind() {
            TokenKind::Not => {
                let op_token = self.advance();
                let operand = self.parse_unary()?;
                let span = Span::merge(op_token.span, expr_span(&operand));
                Ok(Expr::UnaryOp(UnOp::Not, Box::new(operand), span))
            }
            TokenKind::Minus => {
                let op_token = self.advance();
                let operand = self.parse_unary()?;
                let span = Span::merge(op_token.span, expr_span(&operand));
                Ok(Expr::UnaryOp(UnOp::Neg, Box::new(operand), span))
            }
            _ => self.parse_call(),
        }
    }

    fn parse_call(&mut self) -> Result<Expr, ParseError> {
        let mut expr = self.parse_primary()?;
        loop {
            if !self.check(&TokenKind::LParen) {
                break;
            }
            let open = self.advance();
            let mut args = Vec::new();
            if !self.check(&TokenKind::RParen) {
                loop {
                    args.push(self.parse_expr()?);
                    if !self.check(&TokenKind::Comma) {
                        break;
                    }
                    self.advance();
                }
            }
            let close = self.expect(&TokenKind::RParen)?;
            match expr {
                Expr::Ident(name, _) => {
                    let span = Span::merge(open.span, close.span);
                    expr = Expr::FnCall(name, args, span);
                }
                _ => {
                    return Err(ParseError::new(
                        "cannot call a non-identifier expression",
                        close.span,
                    ));
                }
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let token = self.token();
        if let TokenKind::Error(msg) = &token.kind {
            let t = self.advance();
            return Err(ParseError::new(format!("lex error: {msg}"), t.span));
        }
        match token.kind {
            TokenKind::Integer(val) => {
                self.advance();
                Ok(Expr::Literal(val, token.span))
            }
            TokenKind::True => {
                self.advance();
                Ok(Expr::Literal("1".to_string(), token.span))
            }
            TokenKind::False => {
                self.advance();
                Ok(Expr::Literal("0".to_string(), token.span))
            }
            TokenKind::Ident(name) => {
                self.advance();
                Ok(Expr::Ident(name, token.span))
            }
            TokenKind::LParen => {
                self.advance();
                let expr = self.parse_expr()?;
                self.expect(&TokenKind::RParen)?;
                Ok(Expr::Paren(Box::new(expr), token.span))
            }
            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = block.span;
                Ok(Expr::Block(block, span))
            }
            _ => Err(ParseError::new(
                format!("unexpected token {:?}", token.kind),
                token.span,
            )),
        }
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let token = self.token();
        match token.kind {
            TokenKind::BoolType => {
                self.advance();
                Ok(Type::Bool)
            }
            TokenKind::Uint(bits) => {
                self.advance();
                Ok(Type::Uint(bits))
            }
            _ => Err(ParseError::new(
                format!("expected type, found {:?}", token.kind),
                token.span,
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(source: &str) -> Result<Program, ParseError> {
        let mut lexer = uno_lexer::Lexer::new(source.to_string());
        let tokens = lexer.tokenize();
        let mut parser = Parser::new(tokens);
        parser.parse_program()
    }

    #[test]
    fn empty_program() {
        let prog = parse("").unwrap();
        assert!(prog.functions.is_empty());
    }

    #[test]
    fn simple_function() {
        let prog = parse("fn main() -> u32 { return 0; }").unwrap();
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        assert_eq!(f.name, "main");
        assert!(f.params.is_empty());
        assert_eq!(f.return_type, Type::Uint(32));
    }

    #[test]
    fn function_with_params() {
        let prog = parse("fn add(a: u32, b: u32) -> u32 { return a + b; }").unwrap();
        assert_eq!(prog.functions.len(), 1);
        let f = &prog.functions[0];
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].name, "a");
        assert_eq!(f.params[1].name, "b");
    }

    #[test]
    fn let_statement() {
        let prog = parse("fn main() -> u32 { let x: u32 = 42; return x; }").unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::Let(name, is_mut, type_, value, _) => {
                assert_eq!(name, "x");
                assert!(!is_mut);
                assert_eq!(type_.as_ref(), Some(&Type::Uint(32)));
                assert!(matches!(value, Expr::Literal(v, _) if v == "42"));
            }
            _ => panic!("expected let stmt"),
        }
    }

    #[test]
    fn if_else() {
        let prog = parse("fn main() -> u32 { if true { return 1; } else { return 2; } }").unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::If(_, then_block, Some(else_block), _) => {
                assert_eq!(then_block.stmts.len(), 1);
                assert_eq!(else_block.stmts.len(), 1);
            }
            _ => panic!("expected if stmt"),
        }
    }

    #[test]
    fn else_if_chain() {
        let src = "fn main() -> u32 { if false { 1 } else if true { 2 } else { 3 } }";
        let prog = parse(src).unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::If(_, then_block, Some(else_block), _) => {
                assert_eq!(then_block.stmts.len(), 1);
                assert!(matches!(&else_block.stmts[0], Stmt::If(..)));
            }
            _ => panic!("expected if stmt"),
        }
    }

    #[test]
    fn binary_ops() {
        let prog = parse("fn main() -> u32 { return 1 + 2 * 3; }").unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::Return(Some(Expr::BinaryOp(_, _, _, _)), _) => {}
            _ => panic!("expected binary op"),
        }
    }

    #[test]
    fn fn_call() {
        let prog = parse("fn main() -> u32 { return foo(1, 2); }").unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::Return(Some(Expr::FnCall(name, args, _)), _) => {
                assert_eq!(name, "foo");
                assert_eq!(args.len(), 2);
            }
            _ => panic!("expected fn call"),
        }
    }

    #[test]
    fn lex_error_propagated() {
        let result = parse("fn main() -> u32 { let x = @; }");
        assert!(result.is_err());
    }

    #[test]
    fn parse_error_expected_type() {
        let result = parse("fn main() -> foo { }");
        assert!(result.is_err());
    }

    #[test]
    fn block_expr() {
        let prog = parse("fn main() -> u32 { let x = { 42 }; return x; }").unwrap();
        let body = &prog.functions[0].body;
        match &body.stmts[0] {
            Stmt::Let(_, _, _, Expr::Block(_, _), _) => {}
            _ => panic!("expected block expr"),
        }
    }

    #[test]
    fn parse_while() {
        let prog = parse("fn main() -> u32 { while true { return 1; } return 0; }").unwrap();
        let body = &prog.functions[0].body;
        assert!(matches!(&body.stmts[0], Stmt::While(..)));
    }

    #[test]
    fn parse_loop() {
        let prog = parse("fn main() -> u32 { loop { break; } }").unwrap();
        let body = &prog.functions[0].body;
        assert!(matches!(&body.stmts[0], Stmt::Loop(..)));
    }

    #[test]
    fn parse_break() {
        let prog = parse("fn main() -> u32 { loop { break; } }").unwrap();
        let body = &prog.functions[0].body;
        if let Stmt::Loop(block, _) = &body.stmts[0] {
            assert!(matches!(&block.stmts[0], Stmt::Break(..)));
        } else {
            panic!("expected loop");
        }
    }

    #[test]
    fn parse_let_mut() {
        let prog = parse("fn main() -> u32 { let mut x: u32 = 0; return x; }").unwrap();
        let body = &prog.functions[0].body;
        if let Stmt::Let(_, is_mut, _, _, _) = &body.stmts[0] {
            assert!(*is_mut);
        } else {
            panic!("expected let stmt");
        }
    }

    #[test]
    fn parse_pub_fn() {
        let prog = parse("pub fn foo() -> u32 { return 1; }").unwrap();
        assert!(prog.functions[0].public);
    }

    #[test]
    fn parse_assignment() {
        let prog = parse("fn main() -> u32 { let mut x: u32 = 0; x = 5; return x; }").unwrap();
        let body = &prog.functions[0].body;
        assert!(matches!(&body.stmts[1], Stmt::Assign(..)));
    }
}

fn expr_span(expr: &Expr) -> Span {
    match expr {
        Expr::Literal(_, s)
        | Expr::Ident(_, s)
        | Expr::BinaryOp(_, _, _, s)
        | Expr::UnaryOp(_, _, s)
        | Expr::Block(_, s)
        | Expr::FnCall(_, _, s)
        | Expr::Paren(_, s) => *s,
    }
}

fn expr_span_for_stmt(stmt: &Stmt) -> Span {
    match stmt {
        Stmt::Let(_, _, _, _, s)
        | Stmt::Assign(_, _, s)
        | Stmt::Return(_, s)
        | Stmt::Expr(_, s)
        | Stmt::If(_, _, _, s)
        | Stmt::While(_, _, s)
        | Stmt::Loop(_, s)
        | Stmt::Break(s) => *s,
    }
}
