
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
            functions.push(self.parse_fn_def()?);
        }
        Ok(Program { functions })
    }

    fn parse_fn_def(&mut self) -> Result<FnDef, ParseError> {
        let start_span = self.token().span;
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
        match self.kind() {
            TokenKind::Let => self.parse_let_stmt(),
            TokenKind::Return => self.parse_return_stmt(),
            TokenKind::If => self.parse_if_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_let_stmt(&mut self) -> Result<Stmt, ParseError> {
        let start = self.token().span;
        self.expect(&TokenKind::Let)?;
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
            false,
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
                let block = Block {
                    stmts: vec![else_if],
                    span: Span::empty(),
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
