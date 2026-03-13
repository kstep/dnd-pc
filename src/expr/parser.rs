use std::{iter::Peekable, marker::PhantomData, str::FromStr};

use crate::expr::{
    Op,
    error::Error,
    tokenizer::{Token, Tokenizer},
};

pub(super) struct Parser<'a, Var> {
    tokens: Peekable<Tokenizer<'a>>,
    _var: PhantomData<Var>,
}

impl<'a, Var: FromStr> From<Tokenizer<'a>> for Parser<'a, Var> {
    fn from(tokens: Tokenizer<'a>) -> Self {
        Self {
            tokens: tokens.peekable(),
            _var: PhantomData,
        }
    }
}

impl<'a, Var: FromStr> Parser<'a, Var> {
    pub fn new(expr: &'a str) -> Self {
        Self::from(Tokenizer::new(expr))
    }

    pub fn parse(&mut self) -> Result<Vec<Op<Var>>, Error> {
        let mut ops = Vec::new();
        self.parse_into(&mut ops)?;
        Ok(ops)
    }

    pub fn parse_into(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        if self.peek().is_none() {
            return Err(Error::EmptyExpression);
        }

        self.parse_assignment(ops)?;

        if let Ok(Some(token)) = self.next() {
            return Err(Error::unexpected_token(token));
        }

        Ok(())
    }

    fn peek(&mut self) -> Option<&Token<'a>> {
        self.tokens.peek().and_then(|r| r.as_ref().ok())
    }

    fn next(&mut self) -> Result<Option<Token<'a>>, Error> {
        self.tokens.next().transpose()
    }

    fn expect(&mut self, expected: &Token) -> Result<(), Error> {
        match self.next()? {
            Some(ref token) if token == expected => Ok(()),
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    // Continue parsing +/- after the first term has been parsed
    fn parse_expr_tail(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.next()?;
                    self.parse_term(ops)?;
                    ops.push(Op::Add);
                }
                Some(Token::Minus) => {
                    self.next()?;
                    self.parse_term(ops)?;
                    ops.push(Op::Sub);
                }
                _ => break,
            }
        }
        Ok(())
    }

    // expr = term (('+' | '-') term)*
    fn parse_expr(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        self.parse_term(ops)?;
        self.parse_expr_tail(ops)
    }

    // term = unary (('*' | '/' | '\') unary)*
    fn parse_term(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        self.parse_unary(ops)?;
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::Mul);
                }
                Some(Token::Slash) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::DivFloor);
                }
                Some(Token::Backslash) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::DivCeil);
                }
                _ => break,
            }
        }
        Ok(())
    }

    // unary = '-' unary | dice
    fn parse_unary(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        if self.peek() == Some(&Token::Minus) {
            self.next()?;
            self.parse_unary(ops)?;
            ops.push(Op::PushConst(-1));
            ops.push(Op::Mul);
            Ok(())
        } else {
            self.parse_dice(ops)
        }
    }

    // dice = atom ('d' atom ('kh' num | 'kl' num)?)?
    // Also handle bare 'd' with implicit 1: d20 = 1d20
    fn parse_dice(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        if self.peek() == Some(&Token::D) {
            self.next()?;
            ops.push(Op::PushConst(1));
            self.parse_atom(ops)?;
            ops.push(Op::Roll);
            self.parse_keep(ops)?;
            return Ok(());
        }

        self.parse_atom(ops)?;

        if self.peek() == Some(&Token::D) {
            self.next()?;
            self.parse_atom(ops)?;
            ops.push(Op::Roll);
            self.parse_keep(ops)?;
        }
        Ok(())
    }

    fn parse_keep(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        match self.peek() {
            Some(Token::Kh) => {
                self.next()?;
                if let Some(&Token::Num(n)) = self.peek() {
                    self.next()?;
                    ops.push(Op::KeepMax(n as u32));
                }
            }
            Some(Token::Kl) => {
                self.next()?;
                if let Some(&Token::Num(n)) = self.peek() {
                    self.next()?;
                    ops.push(Op::KeepMin(n as u32));
                }
            }
            Some(Token::Dh) => {
                self.next()?;
                if let Some(&Token::Num(n)) = self.peek() {
                    self.next()?;
                    ops.push(Op::DropMax(n as u32));
                }
            }
            Some(Token::Dl) => {
                self.next()?;
                if let Some(&Token::Num(n)) = self.peek() {
                    self.next()?;
                    ops.push(Op::DropMin(n as u32));
                }
            }
            _ => ops.push(Op::Sum),
        }
        Ok(())
    }

    // atom = num | var | func '(' args ')' | '(' expr ')'
    fn parse_atom(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        match self.next()? {
            Some(Token::Num(n)) => {
                ops.push(Op::PushConst(n));
                Ok(())
            }
            Some(Token::Ident(name)) => {
                if let Ok(var) = name.parse() {
                    ops.push(Op::PushVar(var));
                    return Ok(());
                }
                self.parse_function_call(name, ops)?;
                Ok(())
            }
            Some(Token::LParen) => {
                self.parse_expr(ops)?;
                self.expect(&Token::RParen)?;
                Ok(())
            }
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    fn parse_function_call(&mut self, name: &str, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        match name {
            "min" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::Min);
            }
            "max" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::Max);
            }
            _ => return Err(Error::unexpected_token(name)),
        }

        Ok(())
    }

    fn parse_binary_function_call(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        self.expect(&Token::LParen)?;
        self.parse_expr(ops)?;
        self.expect(&Token::Comma)?;
        self.parse_expr(ops)?;
        self.expect(&Token::RParen)?;
        Ok(())
    }

    // assignment = IDENT '=' expr | expr
    fn parse_assignment(&mut self, ops: &mut Vec<Op<Var>>) -> Result<(), Error> {
        loop {
            if let Some(&Token::Ident(name)) = self.peek()
                && let Ok(var) = name.parse::<Var>()
            {
                // Speculatively consume ident, check for '='
                self.next()?;
                if self.peek() == Some(&Token::Eq) {
                    self.next()?;
                    self.parse_expr(ops)?;
                    ops.push(Op::Assign(var));
                } else {
                    // Not an assignment, push the var and continue as expr
                    ops.push(Op::PushVar(var));
                    self.parse_expr_tail(ops)?;
                }
            } else {
                // Not an assignment, parse as expr
                self.parse_expr(ops)?;
            }

            if let Some(&Token::Semicolon) = self.peek() {
                self.next()?;
                // Continue parsing another assignment/expression
                continue;
            }

            break;
        }

        Ok(())
    }
}
