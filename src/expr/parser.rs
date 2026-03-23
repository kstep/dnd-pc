use std::{iter::Peekable, marker::PhantomData, ops::Neg, str::FromStr};

use crate::expr::{
    Op,
    error::Error,
    ops::Cmp,
    tokenizer::{Token, Tokenizer},
};

pub(super) struct Parser<'a, Var, Val> {
    tokens: Peekable<Tokenizer<'a>>,
    /// Extra blocks for sub-expressions (if branches, etc.).
    /// Block indices are 1-based (0 = main block / "no block").
    blocks: Vec<Vec<Op<Var, Val>>>,
    _var: PhantomData<(Var, Val)>,
}

impl<'a, Var, Val> From<Tokenizer<'a>> for Parser<'a, Var, Val> {
    fn from(tokens: Tokenizer<'a>) -> Self {
        Self {
            tokens: tokens.peekable(),
            blocks: Vec::new(),
            _var: PhantomData,
        }
    }
}

impl<'a, Var: FromStr + Copy, Val: FromStr + Copy + Neg<Output = Val>> Parser<'a, Var, Val> {
    pub fn new(expr: &'a str) -> Self {
        Self::from(Tokenizer::new(expr))
    }

    pub fn parse(&mut self) -> Result<Vec<Vec<Op<Var, Val>>>, Error> {
        let mut ops = Vec::new();
        self.parse_into(&mut ops)?;
        let mut blocks = Vec::with_capacity(1 + self.blocks.len());
        blocks.push(ops);
        blocks.append(&mut self.blocks);
        Ok(blocks)
    }

    pub fn parse_into(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
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

    fn expect(&mut self, expected: impl FnOnce(&Token<'a>) -> bool) -> Result<(), Error> {
        match self.next()? {
            Some(ref token) if expected(token) => Ok(()),
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    // or = and ('or' and)*
    fn parse_or(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.parse_and(ops)?;
        self.parse_or_tail(ops)
    }

    fn parse_or_tail(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        while let Some(Token::Or) = self.peek() {
            self.next()?;
            self.parse_and(ops)?;
            ops.push(Op::Or);
        }
        Ok(())
    }

    // and = comparison ('and' comparison)*
    fn parse_and(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.parse_comparison(ops)?;
        self.parse_and_tail(ops)
    }

    fn parse_and_tail(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        while let Some(Token::And) = self.peek() {
            self.next()?;
            self.parse_comparison(ops)?;
            ops.push(Op::And);
        }
        Ok(())
    }

    // comparison = expr (('<' | '>' | '<=' | '>=' | '==' | '!=') expr)?
    fn parse_comparison(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.parse_expr(ops)?;
        self.parse_comparison_tail(ops)
    }

    fn parse_comparison_tail(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        let cmp_op = match self.peek() {
            Some(Token::Lt) => Some(Op::Cmp(Cmp::Lt)),
            Some(Token::Gt) => Some(Op::Cmp(Cmp::Gt)),
            Some(Token::Le) => Some(Op::Cmp(Cmp::Le)),
            Some(Token::Ge) => Some(Op::Cmp(Cmp::Ge)),
            Some(Token::EqEq) => Some(Op::Cmp(Cmp::Eq)),
            Some(Token::NotEq) => Some(Op::Cmp(Cmp::Ne)),
            _ => None,
        };
        if let Some(op) = cmp_op {
            self.next()?;
            self.parse_expr(ops)?;
            ops.push(op);
        }
        Ok(())
    }

    // Continue parsing +/- after the first term has been parsed
    fn parse_expr_tail(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
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
    fn parse_expr(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.parse_term(ops)?;
        self.parse_expr_tail(ops)
    }

    // term = unary (('*' | '/' | '\') unary)*
    fn parse_term(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
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
                Some(Token::Percent) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::Mod);
                }
                _ => break,
            }
        }
        Ok(())
    }

    // unary = '-' unary | 'not' unary | dice
    fn parse_unary(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        if let Some(Token::Not) = self.peek() {
            self.next()?;
            self.parse_unary(ops)?;
            ops.push(Op::Not);
            return Ok(());
        }
        if let Some(Token::Minus) = self.peek() {
            self.next()?;
            if let Some(&Token::Value(n)) = self.peek() {
                let n = parse_value::<Val>(n)?;
                self.next()?;
                ops.push(Op::PushConst(n.neg()));
                Ok(())
            } else {
                self.parse_unary(ops)?;
                let n = parse_value::<Val>("-1")?;
                ops.push(Op::PushConst(n));
                ops.push(Op::Mul);
                Ok(())
            }
        } else {
            self.parse_dice(ops)
        }
    }

    // dice = atom ('d' atom ('kh' num | 'kl' num)?)?
    // Also handle bare 'd' with implicit 1: d20 = 1d20
    fn parse_dice(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        if let Some(Token::D) = self.peek() {
            self.next()?;
            let n = parse_value("1")?;
            ops.push(Op::PushConst(n));
            self.parse_atom(ops)?;
            ops.push(Op::Roll);
            self.parse_keep(ops)?;
            return Ok(());
        }

        self.parse_atom(ops)?;

        if let Some(Token::D) = self.peek() {
            self.next()?;
            self.parse_atom(ops)?;
            ops.push(Op::Roll);
            self.parse_keep(ops)?;
        }
        Ok(())
    }

    fn parse_keep(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        match self.peek() {
            Some(Token::Kh) => {
                self.next()?;
                if let Some(&Token::Value(n)) = self.peek() {
                    let n = parse_value(n)?;
                    self.next()?;
                    ops.push(Op::KeepMax(n));
                } else {
                    ops.push(Op::Sum);
                }
            }
            Some(Token::Kl) => {
                self.next()?;
                if let Some(&Token::Value(n)) = self.peek() {
                    let n = parse_value(n)?;
                    self.next()?;
                    ops.push(Op::KeepMin(n));
                } else {
                    ops.push(Op::Sum);
                }
            }
            Some(Token::Dh) => {
                self.next()?;
                if let Some(&Token::Value(n)) = self.peek() {
                    let n = parse_value(n)?;
                    self.next()?;
                    ops.push(Op::DropMax(n));
                } else {
                    ops.push(Op::Sum);
                }
            }
            Some(Token::Dl) => {
                self.next()?;
                if let Some(&Token::Value(n)) = self.peek() {
                    let n = parse_value(n)?;
                    self.next()?;
                    ops.push(Op::DropMin(n));
                } else {
                    ops.push(Op::Sum);
                }
            }
            _ => ops.push(Op::Sum),
        }
        Ok(())
    }

    // atom = num | var | func '(' args ')' | '(' expr ')'
    fn parse_atom(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        match self.next()? {
            Some(Token::Value(n)) => {
                let n = parse_value(n)?;
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
                self.parse_assignment(ops)?;
                self.expect(|token| matches!(token, Token::RParen))?;
                Ok(())
            }
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    fn parse_function_call(
        &mut self,
        name: &str,
        ops: &mut Vec<Op<Var, Val>>,
    ) -> Result<(), Error> {
        match name {
            "min" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::Min);
            }
            "max" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::Max);
            }
            "avg_hp" | "not" => {
                self.parse_unary_function_call(ops)?;
                ops.push(if name == "not" { Op::Not } else { Op::AvgHp });
            }
            "in" => {
                self.parse_in(ops)?;
            }
            "if" => {
                self.parse_if(ops)?;
            }
            _ => return Err(Error::unexpected_token(name)),
        }

        Ok(())
    }

    fn parse_binary_function_call(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::Comma))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::RParen))?;
        Ok(())
    }

    /// `in(a, b, c)` → `b <= a and a <= c`
    fn parse_in(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::Comma))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::Comma))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::RParen))?;
        ops.push(Op::In);
        Ok(())
    }

    fn parse_if(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        // Condition → Eval(cond_block) pushes result onto stack
        let cond_block = self.parse_sub_block()?;
        ops.push(Op::Eval(cond_block));
        self.expect(|token| matches!(token, Token::Comma))?;
        let then_block = self.parse_sub_block()?;
        // Optional else-branch (0 = noop)
        let else_block = if let Some(Token::Comma) = self.peek() {
            self.next()?;
            self.parse_sub_block()?
        } else {
            0
        };
        self.expect(|token| matches!(token, Token::RParen))?;
        // EvalIf pops cond, branches to then or else block
        ops.push(Op::EvalIf(then_block, else_block));
        Ok(())
    }

    fn parse_sub_block(&mut self) -> Result<u8, Error> {
        let mut block_ops = Vec::new();
        self.parse_assignment(&mut block_ops)?;
        // 1-based: block 0 is reserved (main block / "no block")
        let idx = self.blocks.len() as u8 + 1;
        self.blocks.push(block_ops);
        Ok(idx)
    }

    fn parse_unary_function_call(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::RParen))?;
        Ok(())
    }

    fn compound_op(token: &Token) -> Option<Op<Var, Val>> {
        match token {
            Token::PlusEq => Some(Op::Add),
            Token::MinusEq => Some(Op::Sub),
            Token::StarEq => Some(Op::Mul),
            Token::SlashEq => Some(Op::DivFloor),
            Token::BackslashEq => Some(Op::DivCeil),
            Token::PercentEq => Some(Op::Mod),
            _ => None,
        }
    }

    // assignment = IDENT '=' expr | IDENT op= expr | expr
    fn parse_assignment(&mut self, ops: &mut Vec<Op<Var, Val>>) -> Result<(), Error> {
        loop {
            if let Some(&Token::Ident(name)) = self.peek()
                && let Ok(var) = name.parse::<Var>()
            {
                // Speculatively consume ident, check for '=' or compound
                self.next()?;
                if let Some(Token::Eq) = self.peek() {
                    self.next()?;
                    self.parse_or(ops)?;
                    ops.push(Op::Assign(var));
                } else if let Some(arith_op) = self.peek().and_then(Self::compound_op) {
                    self.next()?;
                    ops.push(Op::PushVar(var));
                    self.parse_or(ops)?;
                    ops.push(arith_op);
                    ops.push(Op::Assign(var));
                } else {
                    // Not an assignment — push var, finish expr, then
                    // handle comparison/boolean tail
                    ops.push(Op::PushVar(var));
                    self.parse_expr_tail(ops)?;
                    self.parse_comparison_tail(ops)?;
                    self.parse_and_tail(ops)?;
                    self.parse_or_tail(ops)?;
                }
            } else {
                // Not an assignment, parse as or_expr
                self.parse_or(ops)?;
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

fn parse_value<Val: FromStr>(token: &str) -> Result<Val, Error> {
    token.parse().map_err(|_| Error::unexpected_token(token))
}
