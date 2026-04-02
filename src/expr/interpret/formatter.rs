use std::fmt;

use super::Interpreter;
use crate::expr::{Error, Op, ops::BlockIndex, stack::Stack};

struct Frag {
    text: String,
    prec: u8, // 0=assign, 1=or, 2=and, 3=cmp, 4=add/sub, 5=mul/div, 6=unary, 7=atom
}

pub struct Formatter {
    stack: Stack<Frag>,
}

impl Formatter {
    pub fn new() -> Self {
        Self {
            stack: Stack::new(),
        }
    }

    fn push(&mut self, text: String, prec: u8) {
        self.stack.push(Frag { text, prec });
    }

    fn wrap(frag: Frag, min_prec: u8) -> String {
        if frag.prec < min_prec {
            format!("({})", frag.text)
        } else {
            frag.text
        }
    }

    fn binary_op(&mut self, sym: &str, prec: u8, right_strict: bool) -> Result<(), Error> {
        let b = self.stack.pop()?;
        let a = self.stack.pop()?;
        let right_min = if right_strict { prec + 1 } else { prec };
        let left = Self::wrap(a, prec);
        let right = Self::wrap(b, right_min);
        self.push(format!("{left} {sym} {right}"), prec);
        Ok(())
    }

    fn binary_func(&mut self, name: &str) -> Result<(), Error> {
        let b = self.stack.pop()?;
        let a = self.stack.pop()?;
        self.push(format!("{name}({}, {})", a.text, b.text), 3);
        Ok(())
    }

    pub fn pop_text(&mut self) -> Result<String, Error> {
        Ok(self.stack.pop()?.text)
    }

    pub fn push_atom(&mut self, text: String) {
        self.push(text, 7);
    }
}

impl<Var: Copy + fmt::Display, Val: Copy + fmt::Display> Interpreter<Var, Val> for Formatter {
    type Output = String;

    fn exec(&mut self, op: Op<Var, Val>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushConst(n) => {
                self.push(n.to_string(), 7);
            }
            Op::PushVar(var) => {
                self.push(var.to_string(), 7);
            }
            Op::Add => self.binary_op("+", 4, false)?,
            Op::Sub => self.binary_op("-", 4, true)?,
            Op::Mul => self.binary_op("*", 5, false)?,
            Op::DivFloor => self.binary_op("/", 5, true)?,
            Op::DivCeil => self.binary_op("\\", 5, true)?,
            Op::Mod => self.binary_op("%", 5, true)?,
            Op::Min => self.binary_func("min")?,
            Op::Max => self.binary_func("max")?,
            Op::Roll => {
                let sides = self.stack.pop()?;
                let count = self.stack.pop()?;
                let sides = Self::wrap(sides, 7);
                let text = if count.text == "1" {
                    format!("d{sides}")
                } else {
                    let count = Self::wrap(count, 7);
                    format!("{count}d{sides}")
                };
                self.push(text, 7);
            }
            Op::Sum => {
                // Follows Roll; roll fragment is already on stack
            }
            Op::Explode => {
                let roll = self.stack.pop()?;
                self.push(format!("{}!", roll.text), 7);
            }
            Op::KeepMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kh{n}", roll.text), 7);
            }
            Op::KeepMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}kl{n}", roll.text), 7);
            }
            Op::DropMax(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dh{n}", roll.text), 7);
            }
            Op::DropMin(n) => {
                let roll = self.stack.pop()?;
                self.push(format!("{}dl{n}", roll.text), 7);
            }
            Op::AvgHp => {
                let sides = self.stack.pop()?;
                self.push(format!("avg_hp({})", sides.text), 7);
            }
            Op::And => self.binary_op("and", 2, false)?,
            Op::Or => self.binary_op("or", 1, false)?,
            Op::Not => {
                let a = self.stack.pop()?;
                let text = Self::wrap(a, 6);
                self.push(format!("not {text}"), 6);
            }
            Op::Cmp(cmp) => self.binary_op(cmp.symbol(), 3, false)?,
            Op::Assign(var) => {
                let val = self.stack.pop()?;
                self.push(format!("{var} = {}", val.text), 0);
            }
            Op::In => {
                let c = self.stack.pop()?;
                let b = self.stack.pop()?;
                let a = self.stack.pop()?;
                self.push(format!("in({}, {}, {})", a.text, b.text, c.text), 3);
            }
            Op::Eval(_) | Op::EvalIf(_, _) => {} // intercepted by format_block
        }
        Ok(None)
    }

    fn finish(self) -> Result<String, Error> {
        Ok(self
            .stack
            .iter()
            .flat_map(|frag| [frag.text.as_str(), "; "])
            .take(self.stack.len() * 2 - 1)
            .collect::<String>())
    }
}
