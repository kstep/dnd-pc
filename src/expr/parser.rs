use std::{iter::Peekable, marker::PhantomData, ops::Neg, str::FromStr};

use crate::expr::{
    Op,
    error::Error,
    group::{NoGroup, VarGroup, VarSubgroup},
    ops::{BLOCK_ERROR, BLOCK_NOOP, BinOp, BlockIndex, Cmp},
    tokenizer::{Token, Tokenizer},
};

pub(super) struct Parser<'a, Var, Val, Grp = NoGroup<Var>> {
    tokens: Peekable<Tokenizer<'a>>,
    /// Extra blocks for sub-expressions (if branches, etc.).
    /// Block indices are 1-based (0 = main block / "no block").
    blocks: Vec<Vec<Op<Var, Val, Grp>>>,
    /// Current `with(@GROUP, ...)` binding. `$` without a name resolves to
    /// this.
    with_binding: Option<VarSubgroup<Grp>>,
    _var: PhantomData<(Var, Val)>,
}

impl<'a, Var, Val, Grp> From<Tokenizer<'a>> for Parser<'a, Var, Val, Grp> {
    fn from(tokens: Tokenizer<'a>) -> Self {
        Self {
            tokens: tokens.peekable(),
            blocks: Vec::new(),
            with_binding: None,
            _var: PhantomData,
        }
    }
}

impl<
    'a,
    Var: FromStr + Copy + PartialEq,
    Val: FromStr + Copy + Neg<Output = Val>,
    Grp: Default + FromStr + Copy + VarGroup<Var = Var>,
> Parser<'a, Var, Val, Grp>
{
    pub fn new(expr: &'a str) -> Self {
        Self::from(Tokenizer::new(expr))
    }

    #[allow(clippy::type_complexity)]
    pub fn parse(&mut self) -> Result<Vec<Vec<Op<Var, Val, Grp>>>, Error> {
        let mut ops = Vec::new();
        self.parse_into(&mut ops)?;
        let mut blocks = Vec::with_capacity(1 + self.blocks.len());
        blocks.push(ops);
        blocks.append(&mut self.blocks);
        Ok(blocks)
    }

    pub fn parse_into(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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
    fn parse_or(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_and(ops)?;
        self.parse_or_tail(ops)
    }

    fn parse_or_tail(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        while let Some(Token::Or) = self.peek() {
            self.next()?;
            self.parse_and(ops)?;
            ops.push(Op::BinOp(BinOp::Or));
        }
        Ok(())
    }

    // and = comparison ('and' comparison)*
    fn parse_and(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_comparison(ops)?;
        self.parse_and_tail(ops)
    }

    fn parse_and_tail(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        while let Some(Token::And) = self.peek() {
            self.next()?;
            self.parse_comparison(ops)?;
            ops.push(Op::BinOp(BinOp::And));
        }
        Ok(())
    }

    // comparison = expr (('<' | '>' | '<=' | '>=' | '==' | '!=') expr)?
    fn parse_comparison(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_expr(ops)?;
        self.parse_comparison_tail(ops)
    }

    fn parse_comparison_tail(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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
    fn parse_expr_tail(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.next()?;
                    self.parse_term(ops)?;
                    ops.push(Op::BinOp(BinOp::Add));
                }
                Some(Token::Minus) => {
                    self.next()?;
                    self.parse_term(ops)?;
                    ops.push(Op::BinOp(BinOp::Sub));
                }
                _ => break,
            }
        }
        Ok(())
    }

    // expr = term (('+' | '-') term)*
    fn parse_expr(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_term(ops)?;
        self.parse_expr_tail(ops)
    }

    // term = unary (('*' | '/' | '\') unary)*
    fn parse_term(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_unary(ops)?;
        self.parse_term_tail(ops)
    }

    fn parse_term_tail(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::BinOp(BinOp::Mul));
                }
                Some(Token::Slash) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::BinOp(BinOp::DivFloor));
                }
                Some(Token::Backslash) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::BinOp(BinOp::DivCeil));
                }
                Some(Token::Percent) => {
                    self.next()?;
                    self.parse_unary(ops)?;
                    ops.push(Op::BinOp(BinOp::Mod));
                }
                _ => break,
            }
        }
        Ok(())
    }

    // unary = '-' unary | 'not' unary | dice
    fn parse_unary(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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
                ops.push(Op::BinOp(BinOp::Mul));
                Ok(())
            }
        } else {
            self.parse_dice(ops)
        }
    }

    // dice = atom ('d' atom ('kh' num | 'kl' num)?)?
    // Also handle bare 'd' with implicit 1: d20 = 1d20
    fn parse_dice(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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

    fn parse_keep(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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
            Some(Token::Bang) => {
                self.next()?;
                ops.push(Op::Explode);
            }
            _ => ops.push(Op::Sum),
        }
        Ok(())
    }

    // atom = num | var | $group | func '(' args ')' | '(' expr ')'
    fn parse_atom(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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
            Some(Token::GroupRef(name)) => {
                let group = parse_group::<Grp>(name)?;
                ops.push(Op::PushGroup(group));
                Ok(())
            }
            Some(Token::At) => {
                let grp = self.with_binding.ok_or(Error::unexpected_token("@"))?.inner;
                ops.push(Op::PushGroup(grp));
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
        ops: &mut Vec<Op<Var, Val, Grp>>,
    ) -> Result<(), Error> {
        match name {
            "min" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::BinOp(BinOp::Min));
            }
            "max" => {
                self.parse_binary_function_call(ops)?;
                ops.push(Op::BinOp(BinOp::Max));
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
            "guard" => {
                self.parse_guard(ops)?;
            }
            "with" => {
                self.parse_with(ops)?;
            }
            "each" => {
                self.parse_each(ops)?;
            }
            "fold" => {
                self.parse_fold(ops)?;
            }
            "tier" => {
                self.parse_tier(ops)?;
            }
            _ => return Err(Error::unexpected_token(name)),
        }

        Ok(())
    }

    fn parse_binary_function_call(
        &mut self,
        ops: &mut Vec<Op<Var, Val, Grp>>,
    ) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::Comma))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::RParen))?;
        Ok(())
    }

    /// `in(a, b, c)` → `b <= a and a <= c`
    fn parse_in(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
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

    fn parse_if(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        // Condition → Eval(cond_block) pushes result onto stack
        let cond_block = self.parse_sub_block()?;
        ops.push(Op::Eval(cond_block));
        self.expect(|token| matches!(token, Token::Comma))?;
        let then_block = self.parse_sub_block()?;
        let else_block = if let Some(Token::Comma) = self.peek() {
            self.next()?;
            self.parse_sub_block()?
        } else {
            BLOCK_NOOP
        };
        self.expect(|token| matches!(token, Token::RParen))?;
        // EvalIf pops cond, branches to then or else block
        ops.push(Op::EvalIf(then_block, else_block));
        Ok(())
    }

    fn parse_guard(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        let cond_block = self.parse_sub_block()?;
        ops.push(Op::Eval(cond_block));
        self.expect(|token| matches!(token, Token::Comma))?;
        let then_block = self.parse_sub_block()?;
        self.expect(|token| matches!(token, Token::RParen))?;
        ops.push(Op::EvalIf(then_block, BLOCK_ERROR));
        Ok(())
    }

    /// `each(@GROUP, body)` →
    /// `[Each(group), EvalIf(m, NOOP)]`
    /// `block m: [...body..., Next(group), EvalIf(m, NOOP)]`
    fn expect_group(&mut self) -> Result<Grp, Error> {
        match self.next()? {
            Some(Token::GroupRef(name)) => parse_group::<Grp>(name),
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    /// Parse `@GROUP` or `@GROUP(elem, elem, ...)` into a VarSubgroup.
    /// Elements are resolved as member names — each must be a valid
    /// `Var` that the group contains, and its index becomes a set bit.
    fn expect_subgroup(&mut self) -> Result<VarSubgroup<Grp>, Error> {
        // Bare $ → use with_binding
        if let Some(Token::At) = self.peek() {
            self.next()?;
            return self.with_binding.ok_or(Error::unexpected_token("@"));
        }
        let group = self.expect_group()?;
        let Some(Token::LParen) = self.peek() else {
            return Ok(group.into());
        };
        self.next()?;
        let mut mask = 0u32;
        loop {
            let name = match self.next()? {
                Some(Token::Ident(name)) => name,
                Some(token) => return Err(Error::unexpected_token(token)),
                None => return Err(Error::UnexpectedEnd),
            };
            let idx = group
                .member_by_name(name)
                .ok_or(Error::unexpected_token(name))?;
            mask |= 1 << idx;
            match self.peek() {
                Some(Token::Comma) => {
                    self.next()?;
                }
                Some(Token::RParen) => {
                    self.next()?;
                    break;
                }
                _ => return Err(Error::UnexpectedEnd),
            }
        }
        Ok(VarSubgroup::masked(group, mask))
    }

    /// `with(@GROUP, body)` — set a group binding so `$` resolves to it.
    fn parse_with(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        let subgrp = self.expect_subgroup()?;
        self.expect(|token| matches!(token, Token::Comma))?;
        let prev = self.with_binding.replace(subgrp);
        self.parse_assignment(ops)?;
        self.with_binding = prev;
        self.expect(|token| matches!(token, Token::RParen))?;
        Ok(())
    }

    fn parse_each(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        let subgrp = self.expect_subgroup()?;
        self.expect(|token| matches!(token, Token::Comma))?;

        let mut body_ops = Vec::new();
        self.parse_assignment(&mut body_ops)?;
        body_ops.push(Op::Next(subgrp));
        let body_idx = self.blocks.len() as BlockIndex + 1;
        body_ops.push(Op::EvalIf(body_idx, BLOCK_NOOP));
        self.blocks.push(body_ops);

        self.expect(|token| matches!(token, Token::RParen))?;
        ops.push(Op::Each(subgrp));
        ops.push(Op::EvalIf(body_idx, BLOCK_NOOP));
        Ok(())
    }

    /// `fold(op, @GROUP, expr)` →
    /// `[Each(group), EvalIf(m, NOOP)]`
    /// `block m: [...expr..., Next(group), EvalIf(n, NOOP)]`
    /// `block n: [Eval(m), BinOp(op)]`
    fn parse_fold(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        let bin_op = self.parse_bin_op()?;
        self.expect(|token| matches!(token, Token::Comma))?;
        let subgrp = self.expect_subgroup()?;

        // Body block (m): expr + Next + EvalIf(n, NOOP)
        let mut body_ops = Vec::new();
        if let Some(Token::Comma) = self.peek() {
            self.next()?;
            self.parse_or(&mut body_ops)?;
        } else {
            // fold(op, @GROUP) shorthand → fold(op, @GROUP, @GROUP)
            body_ops.push(Op::PushGroup(subgrp.inner));
        }
        body_ops.push(Op::Next(subgrp));
        let body_idx = self.blocks.len() as BlockIndex + 1;
        let acc_idx = body_idx + 1;
        body_ops.push(Op::EvalIf(acc_idx, BLOCK_NOOP));
        self.blocks.push(body_ops);

        // Accumulator block (n): Eval(m) + BinOp
        let acc_ops = vec![Op::Eval(body_idx), Op::BinOp(bin_op)];
        self.blocks.push(acc_ops);

        self.expect(|token| matches!(token, Token::RParen))?;
        ops.push(Op::Each(subgrp));
        ops.push(Op::EvalIf(body_idx, BLOCK_NOOP));
        Ok(())
    }

    /// `tier(var, threshold:value, threshold:value, ...)`
    /// `tier(var, threshold:value, threshold:value, ...)`
    fn parse_tier(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        // First argument is always a variable identifier
        let var: Var = match self.next()? {
            Some(Token::Ident(name)) => name.parse().map_err(|_| Error::unexpected_token(name))?,
            Some(token) => return Err(Error::unexpected_token(token)),
            None => return Err(Error::UnexpectedEnd),
        };
        // Parse threshold:value pairs
        let mut count: u8 = 0;
        while matches!(self.peek(), Some(Token::Comma)) {
            self.next()?;
            if matches!(self.peek(), Some(Token::RParen)) {
                break;
            }
            self.parse_or(ops)?;
            self.expect(|token| matches!(token, Token::Colon))?;
            self.parse_or(ops)?;
            count += 1;
        }
        self.expect(|token| matches!(token, Token::RParen))?;
        if count == 0 {
            return Err(Error::unexpected_token(")"));
        }
        ops.push(Op::PushVar(var));
        ops.push(Op::Tier(count));
        Ok(())
    }

    fn parse_bin_op(&mut self) -> Result<BinOp, Error> {
        match self.next()? {
            Some(Token::Plus) => Ok(BinOp::Add),
            Some(Token::Minus) => Ok(BinOp::Sub),
            Some(Token::Star) => Ok(BinOp::Mul),
            Some(Token::Slash) => Ok(BinOp::DivFloor),
            Some(Token::Backslash) => Ok(BinOp::DivCeil),
            Some(Token::Percent) => Ok(BinOp::Mod),
            Some(Token::And) => Ok(BinOp::And),
            Some(Token::Or) => Ok(BinOp::Or),
            Some(Token::Ident("min")) => Ok(BinOp::Min),
            Some(Token::Ident("max")) => Ok(BinOp::Max),
            Some(token) => Err(Error::unexpected_token(token)),
            None => Err(Error::UnexpectedEnd),
        }
    }

    fn parse_sub_block(&mut self) -> Result<BlockIndex, Error> {
        let mut block_ops = Vec::new();
        self.parse_assignment(&mut block_ops)?;
        let idx = self.blocks.len() as BlockIndex + 1;
        self.blocks.push(block_ops);
        Ok(idx)
    }

    fn parse_unary_function_call(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.expect(|token| matches!(token, Token::LParen))?;
        self.parse_expr(ops)?;
        self.expect(|token| matches!(token, Token::RParen))?;
        Ok(())
    }

    fn compound_op(token: &Token) -> Option<BinOp> {
        match token {
            Token::PlusEq => Some(BinOp::Add),
            Token::MinusEq => Some(BinOp::Sub),
            Token::StarEq => Some(BinOp::Mul),
            Token::SlashEq => Some(BinOp::DivFloor),
            Token::BackslashEq => Some(BinOp::DivCeil),
            Token::PercentEq => Some(BinOp::Mod),
            _ => None,
        }
    }

    /// Parse remaining expression from term level up: term → expr → cmp → bool.
    fn parse_expr_from_term(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        self.parse_term_tail(ops)?;
        self.parse_expr_tail(ops)?;
        self.parse_comparison_tail(ops)?;
        self.parse_and_tail(ops)?;
        self.parse_or_tail(ops)
    }

    /// Parse `target = expr`, `target op= expr`, or fall through to expression.
    fn parse_assign_target(
        &mut self,
        ops: &mut Vec<Op<Var, Val, Grp>>,
        push_op: Op<Var, Val, Grp>,
        assign_op: Op<Var, Val, Grp>,
    ) -> Result<(), Error>
    where
        Op<Var, Val, Grp>: Copy,
    {
        if let Some(Token::Eq) = self.peek() {
            self.next()?;
            self.parse_or(ops)?;
            ops.push(assign_op);
        } else if let Some(bin_op) = self.peek().and_then(Self::compound_op) {
            self.next()?;
            ops.push(push_op);
            self.parse_or(ops)?;
            ops.push(Op::BinOp(bin_op));
            ops.push(assign_op);
        } else {
            ops.push(push_op);
            self.parse_expr_from_term(ops)?;
        }
        Ok(())
    }

    fn parse_assignment(&mut self, ops: &mut Vec<Op<Var, Val, Grp>>) -> Result<(), Error> {
        loop {
            if let Some(Token::At) = self.peek() {
                let group = self.with_binding.ok_or(Error::unexpected_token("@"))?.inner;
                self.next()?;
                self.parse_assign_target(ops, Op::PushGroup(group), Op::AssignGroup(group))?;
            } else if let Some(&Token::GroupRef(name)) = self.peek() {
                let group = parse_group::<Grp>(name)?;
                self.next()?;
                self.parse_assign_target(ops, Op::PushGroup(group), Op::AssignGroup(group))?;
            } else if let Some(&Token::Ident(name)) = self.peek()
                && let Ok(var) = name.parse::<Var>()
            {
                self.next()?;
                if let Some(Token::Eq) = self.peek() {
                    self.next()?;
                    self.parse_or(ops)?;
                    ops.push(Op::AssignVar(var));
                } else if let Some(bin_op) = self.peek().and_then(Self::compound_op) {
                    self.next()?;
                    ops.push(Op::PushVar(var));
                    self.parse_or(ops)?;
                    ops.push(Op::BinOp(bin_op));
                    ops.push(Op::AssignVar(var));
                } else {
                    // Not an assignment — push var, finish expr from
                    // atom level up: dice → term → expr → cmp → bool
                    ops.push(Op::PushVar(var));
                    // dice tail: var may be followed by 'd' (e.g. SLOT_LEVEL d6)
                    if let Some(Token::D) = self.peek() {
                        self.next()?;
                        self.parse_atom(ops)?;
                        ops.push(Op::Roll);
                        self.parse_keep(ops)?;
                    }
                    self.parse_expr_from_term(ops)?;
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

fn parse_group<Grp: FromStr>(name: &str) -> Result<Grp, Error> {
    name.parse().map_err(|_| Error::unexpected_token(name))
}
