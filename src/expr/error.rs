use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    UnexpectedChar(char),
    UnexpectedEnd,
    UnexpectedToken(Box<str>),
    StackUnderflow,
    EmptyExpression,
    DivisionByZero,
    ReadOnlyVar(Box<str>),
    AssignAtEval(Box<str>),
    UnsupportedVar(Box<str>),
    DicePoolExhausted(u32),
    InvalidDieSides(i32),
    InvalidBlock(u8),
    RngFailed,
}

impl Error {
    pub fn unsupported_var<T: fmt::Display>(var: T) -> Self {
        Self::UnsupportedVar(format!("{var}").into_boxed_str())
    }

    pub fn unexpected_token<T: fmt::Debug>(token: T) -> Self {
        Self::UnexpectedToken(format!("{token:?}").into_boxed_str())
    }

    pub fn read_only_var<T: fmt::Display>(var: T) -> Self {
        Self::ReadOnlyVar(format!("{var}").into_boxed_str())
    }

    pub fn assign_at_eval<T: fmt::Display>(var: T) -> Self {
        Self::AssignAtEval(format!("{var}").into_boxed_str())
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
            Error::ReadOnlyVar(var) => write!(f, "cannot assign to read-only variable: {var}"),
            Error::UnsupportedVar(var) => write!(f, "unsupported variable: {var}"),
            Error::AssignAtEval(var) => {
                write!(f, "cannot assign to field during evaluation: {var}")
            }
            Error::DicePoolExhausted(sides) => write!(f, "dice pool exhausted for d{sides}"),
            Error::InvalidDieSides(sides) => write!(f, "invalid die sides: {sides}"),
            Error::InvalidBlock(idx) => write!(f, "invalid block index: {idx}"),
            Error::RngFailed => write!(f, "random number generation failed"),
        }
    }
}

impl std::error::Error for Error {}
