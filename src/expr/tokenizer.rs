use crate::expr::error::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Token<'a> {
    Value(&'a str),
    Ident(&'a str),
    Plus,
    Minus,
    Star,
    Slash,
    Backslash,
    D,
    Kh,
    Kl,
    Dh,
    Dl,
    Percent,
    Eq,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    BackslashEq,
    PercentEq,
    LParen,
    RParen,
    Comma,
    Semicolon,
    Bang,
    // Boolean / comparison
    And,
    Or,
    Not,
    Lt,
    Gt,
    Le,
    Ge,
    EqEq,
    NotEq,
}

pub(super) struct Tokenizer<'a> {
    rest: &'a str,
}

impl<'a> Tokenizer<'a> {
    pub fn new(input: &'a str) -> Self {
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
                Some(Ok(Token::Value(digits)))
            }
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                // Backtick-quoted segments (e.g. FEAT.`Spellcasting (Bard)`)
                // are included as part of the identifier token.
                let mut in_backtick = false;
                let len = self
                    .rest
                    .bytes()
                    .take_while(|&b| {
                        if b == b'`' {
                            in_backtick = !in_backtick;
                            true
                        } else {
                            in_backtick || b.is_ascii_alphanumeric() || b == b'_' || b == b'.'
                        }
                    })
                    .count();
                if in_backtick {
                    return Some(Err(Error::UnexpectedChar('`')));
                }
                let (ident, rest) = self.rest.split_at(len);
                // Dice keywords glued to digits ("d6", "kh3", "dl1") must be
                // split: emit the keyword token now and leave the digits for the
                // next iteration.
                let (dice_tok, dice_len) = match ident.as_bytes() {
                    [b'k', b'h', b'0'..=b'9', ..] => (Some(Token::Kh), 2),
                    [b'k', b'l', b'0'..=b'9', ..] => (Some(Token::Kl), 2),
                    [b'd', b'h', b'0'..=b'9', ..] => (Some(Token::Dh), 2),
                    [b'd', b'l', b'0'..=b'9', ..] => (Some(Token::Dl), 2),
                    [b'd', b'0'..=b'9', ..] => (Some(Token::D), 1),
                    _ => (None, 0),
                };
                if let Some(tok) = dice_tok {
                    // Advance past keyword only, leaving digits + remaining input.
                    // self.rest still points to the full input before split_at,
                    // so skip just the keyword length.
                    self.rest = &self.rest[dice_len..];
                    return Some(Ok(tok));
                }
                self.rest = rest;
                Some(Ok(match ident {
                    "d" => Token::D,
                    "kh" => Token::Kh,
                    "kl" => Token::Kl,
                    "dh" => Token::Dh,
                    "dl" => Token::Dl,
                    "and" => Token::And,
                    "or" => Token::Or,
                    "not" => Token::Not,
                    _ => Token::Ident(ident),
                }))
            }
            b'+' | b'-' | b'*' | b'/' | b'\\' | b'%' | b'(' | b')' | b',' | b'=' | b';' | b'<'
            | b'>' | b'!' => {
                let second = self.rest.as_bytes().get(1).copied();
                if second == Some(b'=') {
                    let tok = match first {
                        b'+' => Some(Token::PlusEq),
                        b'-' => Some(Token::MinusEq),
                        b'*' => Some(Token::StarEq),
                        b'/' => Some(Token::SlashEq),
                        b'\\' => Some(Token::BackslashEq),
                        b'%' => Some(Token::PercentEq),
                        b'=' => Some(Token::EqEq),
                        b'!' => Some(Token::NotEq),
                        b'<' => Some(Token::Le),
                        b'>' => Some(Token::Ge),
                        _ => None,
                    };
                    if let Some(tok) = tok {
                        self.rest = &self.rest[2..];
                        return Some(Ok(tok));
                    }
                }
                self.rest = &self.rest[1..];
                Some(Ok(match first {
                    b'+' => Token::Plus,
                    b'-' => Token::Minus,
                    b'*' => Token::Star,
                    b'/' => Token::Slash,
                    b'\\' => Token::Backslash,
                    b'%' => Token::Percent,
                    b'=' => Token::Eq,
                    b'<' => Token::Lt,
                    b'>' => Token::Gt,
                    b'!' => Token::Bang,
                    b'(' => Token::LParen,
                    b')' => Token::RParen,
                    b',' => Token::Comma,
                    b';' => Token::Semicolon,
                    _ => return Some(Err(Error::UnexpectedChar(first as char))),
                }))
            }
            _ => Some(Err(Error::UnexpectedChar(first as char))),
        }
    }
}
