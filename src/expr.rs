use std::{fmt, iter::Peekable, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Deserializer, Serialize, de};

pub trait Context<Var> {
    fn assign(&mut self, var: Var, value: i32) -> Result<(), Error>;
    fn resolve(&self, var: Var) -> Result<i32, Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Expr<Var> {
    ops: Vec<Op<Var>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnexpectedChar(char),
    UnexpectedEnd,
    UnexpectedToken(Box<str>),
    StackUnderflow,
    EmptyExpression,
    DivisionByZero,
    ReadOnlyField(Box<str>),
    AssignAtEval(Box<str>),
}

impl Error {
    pub fn unexpected_token<T: fmt::Debug>(token: T) -> Self {
        Self::UnexpectedToken(format!("{token:?}").into_boxed_str())
    }

    pub fn read_only_field<T: fmt::Debug>(var: T) -> Self {
        Self::ReadOnlyField(format!("{var:?}").into_boxed_str())
    }

    pub fn assign_at_eval<T: fmt::Debug>(var: T) -> Self {
        Self::AssignAtEval(format!("{var:?}").into_boxed_str())
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::UnexpectedChar(ch) => write!(f, "unexpected character: '{ch}'"),
            Error::UnexpectedEnd => write!(f, "unexpected end of expression"),
            Error::UnexpectedToken(token) => write!(f, "unexpected token: {token}"),
            Error::StackUnderflow => write!(f, "stack underflow"),
            Error::EmptyExpression => write!(f, "empty expression"),
            Error::DivisionByZero => write!(f, "division by zero"),
            Error::ReadOnlyField(var) => write!(f, "cannot assign to read-only field: {var}"),
            Error::AssignAtEval(var) => {
                write!(f, "cannot assign to field during evaluation: {var}")
            }
        }
    }
}

impl std::error::Error for Error {}

/// Generate a random number in 1..=sides using getrandom.
fn roll_die(sides: i32) -> i32 {
    let n = getrandom::u32().unwrap();
    (n % sides as u32 + 1) as i32
}

struct Stack(Vec<i32>);

impl Stack {
    const DEFAULT_CAPACITY: usize = 16;

    fn new() -> Self {
        Self(Vec::with_capacity(Self::DEFAULT_CAPACITY))
    }

    fn push(&mut self, val: i32) {
        self.0.push(val);
    }

    fn pop(&mut self) -> Result<i32, Error> {
        self.0.pop().ok_or(Error::StackUnderflow)
    }

    fn pop2(&mut self) -> Result<(i32, i32), Error> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }

    /// Pop `count` items, apply `f` to the slice, push the result.
    fn pop_n_reduce(
        &mut self,
        count: usize,
        f: impl FnOnce(&mut [i32]) -> i32,
    ) -> Result<(), Error> {
        let start = self
            .0
            .len()
            .checked_sub(count)
            .ok_or(Error::StackUnderflow)?;
        let result = f(&mut self.0[start..]);
        self.0.truncate(start);
        self.0.push(result);
        Ok(())
    }

    fn result(mut self) -> Result<i32, Error> {
        self.pop()
    }
}

impl<Var: Copy + fmt::Debug> Expr<Var> {
    pub fn apply(&self, ctx: &mut impl Context<Var>) -> Result<i32, Error> {
        let mut stack = Stack::new();
        for &op in &self.ops {
            match op {
                Op::PushVar(var) => stack.push(ctx.resolve(var)?),
                Op::Assign(var) => {
                    let value = *stack.0.last().ok_or(Error::StackUnderflow)?;
                    ctx.assign(var, value)?;
                }
                op => Self::eval_op(&mut stack, op)?,
            }
        }
        stack.result()
    }

    pub fn eval(&self, ctx: &impl Context<Var>) -> Result<i32, Error> {
        let mut stack = Stack::new();
        for &op in &self.ops {
            match op {
                Op::PushVar(var) => stack.push(ctx.resolve(var)?),
                Op::Assign(var) => return Err(Error::assign_at_eval(var)),
                op => Self::eval_op(&mut stack, op)?,
            }
        }
        stack.result()
    }

    fn eval_op(stack: &mut Stack, op: Op<Var>) -> Result<(), Error> {
        match op {
            Op::PushConst(n) => stack.push(n),
            Op::Add => {
                let (a, b) = stack.pop2()?;
                stack.push(a + b);
            }
            Op::Sub => {
                let (a, b) = stack.pop2()?;
                stack.push(a - b);
            }
            Op::Mul => {
                let (a, b) = stack.pop2()?;
                stack.push(a * b);
            }
            Op::DivFloor => {
                let (a, b) = stack.pop2()?;
                if b == 0 {
                    return Err(Error::DivisionByZero);
                }
                stack.push(a.div_euclid(b));
            }
            Op::DivCeil => {
                let (a, b) = stack.pop2()?;
                if b == 0 {
                    return Err(Error::DivisionByZero);
                }
                stack.push((a + b - 1).div_euclid(b));
            }
            Op::Min => {
                let (a, b) = stack.pop2()?;
                stack.push(a.min(b));
            }
            Op::Max => {
                let (a, b) = stack.pop2()?;
                stack.push(a.max(b));
            }
            Op::Roll => {
                let (count, sides) = stack.pop2()?;
                for _ in 0..count {
                    stack.push(roll_die(sides));
                }
                stack.push(count);
            }
            Op::KeepMax(n) => {
                let count = stack.pop()? as usize;
                stack.pop_n_reduce(count, |vals| {
                    vals.sort_unstable_by(|a, b| b.cmp(a));
                    vals[..n as usize].iter().sum()
                })?;
            }
            Op::KeepMin(n) => {
                let count = stack.pop()? as usize;
                stack.pop_n_reduce(count, |vals| {
                    vals.sort_unstable();
                    vals[..n as usize].iter().sum()
                })?;
            }
            Op::Sum => {
                let count = stack.pop()? as usize;
                stack.pop_n_reduce(count, |vals| vals.iter().sum())?;
            }
            Op::PushVar(_) | Op::Assign(_) => unreachable!(),
        }
        Ok(())
    }
}

// --- Tokenizer ---

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token<'a> {
    Num(i32),
    Ident(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    Backslash,
    D,
    Kh,
    Kl,
    Eq,
    LParen,
    RParen,
    Comma,
}

struct Tokenizer<'a> {
    rest: &'a str,
}

impl<'a> Tokenizer<'a> {
    fn new(input: &'a str) -> Self {
        Self { rest: input }
    }
}

impl<'a> Iterator for Tokenizer<'a> {
    type Item = Result<Token<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        self.rest = self.rest.trim_ascii_start();
        let &first = self.rest.as_bytes().first()?;
        match first {
            b'0'..=b'9' => {
                let len = self.rest.bytes().take_while(|b| b.is_ascii_digit()).count();
                let (digits, rest) = self.rest.split_at(len);
                self.rest = rest;
                Some(
                    digits
                        .parse()
                        .map(Token::Num)
                        .map_err(|_| Error::unexpected_token(digits)),
                )
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                let len = self
                    .rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphabetic() || *b == b'_')
                    .count();
                let (ident, rest) = self.rest.split_at(len);
                self.rest = rest;
                Some(Ok(match ident {
                    "d" => Token::D,
                    "kh" => Token::Kh,
                    "kl" => Token::Kl,
                    _ => Token::Ident(ident),
                }))
            }
            b'+' | b'-' | b'*' | b'/' | b'\\' | b'(' | b')' | b',' | b'=' => {
                self.rest = &self.rest[1..];
                Some(Ok(match first {
                    b'+' => Token::Plus,
                    b'-' => Token::Minus,
                    b'*' => Token::Star,
                    b'/' => Token::Slash,
                    b'\\' => Token::Backslash,
                    b'=' => Token::Eq,
                    b'(' => Token::LParen,
                    b')' => Token::RParen,
                    b',' => Token::Comma,
                    _ => unreachable!(),
                }))
            }
            _ => Some(Err(Error::UnexpectedChar(first as char))),
        }
    }
}

// --- Parser (recursive descent, infix to postfix) ---

struct Parser<'a, Var> {
    tokens: Peekable<Tokenizer<'a>>,
    _var: PhantomData<Var>,
}

impl<'a, Var: FromStr> Parser<'a, Var> {
    fn new(tokens: Tokenizer<'a>) -> Self {
        Self {
            tokens: tokens.peekable(),
            _var: PhantomData,
        }
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
                if let Some(var) = name.parse().ok() {
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
        if let Some(&Token::Ident(name)) = self.peek() {
            if let Ok(var) = name.parse::<Var>() {
                // Speculatively consume ident, check for '='
                self.next()?;
                if self.peek() == Some(&Token::Eq) {
                    self.next()?;
                    self.parse_expr(ops)?;
                    ops.push(Op::Assign(var));
                    return Ok(());
                }
                // Not an assignment, push the var and continue as expr
                ops.push(Op::PushVar(var));
                return self.parse_expr_tail(ops);
            }
        }
        self.parse_expr(ops)
    }
}

impl<Var: FromStr> FromStr for Expr<Var> {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parser = Parser::new(Tokenizer::new(s));
        let mut ops = Vec::new();
        if parser.peek().is_none() {
            return Err(Error::EmptyExpression);
        }
        parser.parse_assignment(&mut ops)?;
        if let Ok(Some(token)) = parser.next() {
            return Err(Error::unexpected_token(token));
        }
        Ok(Expr { ops })
    }
}

impl<'de, Var: FromStr + Deserialize<'de>> Deserialize<'de> for Expr<Var> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ExprVisitor<Var>(PhantomData<Var>);

        impl<'de, Var: FromStr + Deserialize<'de>> de::Visitor<'de> for ExprVisitor<Var> {
            type Value = Expr<Var>;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("an expression string or a sequence of ops")
            }

            fn visit_str<E: de::Error>(self, s: &str) -> Result<Expr<Var>, E> {
                s.parse().map_err(de::Error::custom)
            }

            fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Expr<Var>, A::Error> {
                let ops = Vec::deserialize(de::value::SeqAccessDeserializer::new(seq))?;
                Ok(Expr { ops })
            }

            fn visit_map<A: de::MapAccess<'de>>(self, map: A) -> Result<Expr<Var>, A::Error> {
                #[derive(serde::Deserialize)]
                struct ExprFields<Var> {
                    ops: Vec<Op<Var>>,
                }
                let fields = ExprFields::deserialize(de::value::MapAccessDeserializer::new(map))?;
                Ok(Expr { ops: fields.ops })
            }
        }

        deserializer.deserialize_any(ExprVisitor(PhantomData))
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Op<Var> {
    PushVar(Var),
    PushConst(i32),
    Add,      // +
    Sub,      // -
    Mul,      // *
    DivFloor, // /
    DivCeil,  // \
    Min,
    Max,
    Roll,         // 2d20 -> 2 20 Roll Sum
    KeepMax(u32), // 2d20kh1 -> 2 20 Roll KeepMax(1)
    KeepMin(u32),
    Sum,
    Assign(Var),
}

impl<Var: fmt::Display> fmt::Display for Expr<Var> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        struct Frag {
            text: String,
            prec: u8, // 0=assign, 1=add/sub, 2=mul/div, 3=atom
        }

        let mut stack: Vec<Frag> = Vec::new();

        for op in &self.ops {
            match op {
                Op::PushConst(n) => {
                    let text = if *n < 0 {
                        format!("({n})")
                    } else {
                        n.to_string()
                    };
                    stack.push(Frag { text, prec: 3 });
                }
                Op::PushVar(var) => {
                    stack.push(Frag {
                        text: var.to_string(),
                        prec: 3,
                    });
                }
                Op::Add | Op::Sub => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    let sym = if matches!(op, Op::Add) { "+" } else { "-" };
                    let left = if a.prec < 1 {
                        format!("({})", a.text)
                    } else {
                        a.text
                    };
                    let right = if b.prec < 1 || (matches!(op, Op::Sub) && b.prec <= 1) {
                        format!("({})", b.text)
                    } else {
                        b.text
                    };
                    stack.push(Frag {
                        text: format!("{left} {sym} {right}"),
                        prec: 1,
                    });
                }
                Op::Mul | Op::DivFloor | Op::DivCeil => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    let sym = match op {
                        Op::Mul => "*",
                        Op::DivFloor => "/",
                        Op::DivCeil => "\\",
                        _ => unreachable!(),
                    };
                    let left = if a.prec < 2 {
                        format!("({})", a.text)
                    } else {
                        a.text
                    };
                    let right = if b.prec < 2
                        || (matches!(op, Op::DivFloor | Op::DivCeil) && b.prec <= 2)
                    {
                        format!("({})", b.text)
                    } else {
                        b.text
                    };
                    stack.push(Frag {
                        text: format!("{left} {sym} {right}"),
                        prec: 2,
                    });
                }
                Op::Min => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    stack.push(Frag {
                        text: format!("min({}, {})", a.text, b.text),
                        prec: 3,
                    });
                }
                Op::Max => {
                    let b = stack.pop().unwrap();
                    let a = stack.pop().unwrap();
                    stack.push(Frag {
                        text: format!("max({}, {})", a.text, b.text),
                        prec: 3,
                    });
                }
                Op::Roll => {
                    let sides = stack.pop().unwrap();
                    let count = stack.pop().unwrap();
                    let text = if count.text == "1" {
                        format!("d{}", sides.text)
                    } else {
                        format!("{}d{}", count.text, sides.text)
                    };
                    stack.push(Frag { text, prec: 3 });
                }
                Op::Sum => {
                    // Follows Roll; roll fragment is already on stack
                }
                Op::KeepMax(n) => {
                    let roll = stack.pop().unwrap();
                    stack.push(Frag {
                        text: format!("{}kh{n}", roll.text),
                        prec: 3,
                    });
                }
                Op::KeepMin(n) => {
                    let roll = stack.pop().unwrap();
                    stack.push(Frag {
                        text: format!("{}kl{n}", roll.text),
                        prec: 3,
                    });
                }
                Op::Assign(var) => {
                    let val = stack.pop().unwrap();
                    stack.push(Frag {
                        text: format!("{var} = {}", val.text),
                        prec: 0,
                    });
                }
            }
        }

        if let Some(top) = stack.pop() {
            f.write_str(&top.text)
        } else {
            Ok(())
        }
    }
}
#[cfg(test)]
mod tests {
    use wasm_bindgen_test::*;

    use super::*;
    use crate::model::{Ability, AbilityScores};

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
    enum Var {
        Modifier(Ability),
        Ac,
    }

    impl fmt::Display for Var {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            match self {
                Var::Modifier(Ability::Strength) => write!(f, "STR"),
                Var::Modifier(Ability::Dexterity) => write!(f, "DEX"),
                Var::Modifier(Ability::Constitution) => write!(f, "CON"),
                Var::Modifier(Ability::Intelligence) => write!(f, "INT"),
                Var::Modifier(Ability::Wisdom) => write!(f, "WIS"),
                Var::Modifier(Ability::Charisma) => write!(f, "CHA"),
                Var::Ac => write!(f, "AC"),
            }
        }
    }

    impl FromStr for Var {
        type Err = ();

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s {
                "STR" => Ok(Var::Modifier(Ability::Strength)),
                "DEX" => Ok(Var::Modifier(Ability::Dexterity)),
                "CON" => Ok(Var::Modifier(Ability::Constitution)),
                "INT" => Ok(Var::Modifier(Ability::Intelligence)),
                "WIS" => Ok(Var::Modifier(Ability::Wisdom)),
                "CHA" => Ok(Var::Modifier(Ability::Charisma)),
                "AC" => Ok(Var::Ac),
                _ => Err(()),
            }
        }
    }

    type Expr = super::Expr<Var>;

    struct Character {
        abilities: AbilityScores,
        ac: i32,
    }

    fn test_character() -> Character {
        Character {
            abilities: AbilityScores {
                strength: 10,
                dexterity: 14,
                constitution: 12,
                intelligence: 9,
                wisdom: 13,
                charisma: 18,
            },
            ac: 15,
        }
    }

    impl Context<Var> for Character {
        fn assign(&mut self, _var: Var, _value: i32) -> Result<(), Error> {
            unimplemented!()
        }

        fn resolve(&self, var: Var) -> Result<i32, Error> {
            match var {
                Var::Modifier(Ability::Strength) => Ok(0),
                Var::Modifier(Ability::Dexterity) => Ok(2),
                Var::Modifier(Ability::Constitution) => Ok(1),
                Var::Modifier(Ability::Intelligence) => Ok(-1),
                Var::Modifier(Ability::Wisdom) => Ok(1),
                Var::Modifier(Ability::Charisma) => Ok(4),
                Var::Ac => Ok(self.ac),
            }
        }
    }

    #[wasm_bindgen_test]
    fn display_expr() {
        let expr: Expr = "10 + CHA + DEX".parse().unwrap();
        assert_eq!(expr.to_string(), "10 + CHA + DEX");

        let expr: Expr = "2 * (3 + 4)".parse().unwrap();
        assert_eq!(expr.to_string(), "2 * (3 + 4)");

        let expr: Expr = "2d6kh1 + 3".parse().unwrap();
        assert_eq!(expr.to_string(), "2d6kh1 + 3");
    }

    #[wasm_bindgen_test]
    fn sorcery_resilience() {
        let ch = test_character();

        // 10 + CHA + DEX
        let expr: Expr = "10 + CHA + DEX".parse().unwrap();
        assert_eq!(
            expr.ops,
            vec![
                Op::PushConst(10),
                Op::PushVar(Var::Modifier(Ability::Charisma)),
                Op::Add,
                Op::PushVar(Var::Modifier(Ability::Dexterity)),
                Op::Add,
            ]
        );

        let value = expr.eval(&ch).unwrap();
        assert_eq!(value, 16);
    }

    #[wasm_bindgen_test]
    fn arithmetic() {
        let ch = test_character();

        let expr: Expr = "2 * 3 + 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 10);

        let expr: Expr = "2 + 3 * 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 14);

        let expr: Expr = "(2 + 3) * 4".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 20);
    }

    #[wasm_bindgen_test]
    fn unary_minus() {
        let ch = test_character();
        let expr: Expr = "-5 + 10".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 5);
    }

    #[wasm_bindgen_test]
    fn min_max() {
        let ch = test_character();

        let expr: Expr = "min(3, 7)".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 3);

        let expr: Expr = "max(3, 7)".parse().unwrap();
        assert_eq!(expr.eval(&ch).unwrap(), 7);
    }

    #[wasm_bindgen_test]
    fn dice_parse() {
        let expr: Expr = "2d6".parse().unwrap();
        assert_eq!(
            expr.ops,
            vec![Op::PushConst(2), Op::PushConst(6), Op::Roll, Op::Sum]
        );

        let expr: Expr = "4d6kh3".parse().unwrap();
        assert_eq!(
            expr.ops,
            vec![Op::PushConst(4), Op::PushConst(6), Op::Roll, Op::KeepMax(3)]
        );
    }

    #[wasm_bindgen_test]
    fn ability_modifiers() {
        let ch = test_character();
        // STR 10 -> mod 0, DEX 14 -> mod 2, CON 12 -> mod 1
        // INT 9 -> mod -1, WIS 13 -> mod 1, CHA 18 -> mod 4
        assert_eq!("STR".parse::<Expr>().unwrap().eval(&ch).unwrap(), 0);
        assert_eq!("DEX".parse::<Expr>().unwrap().eval(&ch).unwrap(), 2);
        assert_eq!("CON".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("INT".parse::<Expr>().unwrap().eval(&ch).unwrap(), -1);
        assert_eq!("WIS".parse::<Expr>().unwrap().eval(&ch).unwrap(), 1);
        assert_eq!("CHA".parse::<Expr>().unwrap().eval(&ch).unwrap(), 4);
    }
}
