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
                let len = self
                    .rest
                    .bytes()
                    .take_while(|b| b.is_ascii_alphabetic() || *b == b'_' || *b == b'.')
                    .count();
                let (ident, rest) = self.rest.split_at(len);
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
