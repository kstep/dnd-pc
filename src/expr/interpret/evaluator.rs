use std::{fmt, marker::PhantomData};

use super::{Interpreter, eval_op};
use crate::expr::{Context, Error, Op, ops::BlockIndex, stack::Stack};

// --- Evaluator (apply mode, mutable context) ---

pub(crate) struct Evaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a mut Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> Evaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for Evaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::Assign(var) => {
                self.ctx.assign(var, *self.stack.top()?)?;
                Ok(None)
            }
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- ReadOnlyEvaluator (eval mode, immutable context) ---

pub(crate) struct ReadOnlyEvaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    ctx: &'a Ctx,
    lenient: bool,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> ReadOnlyEvaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            lenient: false,
            _var: PhantomData,
        }
    }

    pub fn lenient(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            ctx,
            lenient: true,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>> Interpreter<Var, i32>
    for ReadOnlyEvaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::Assign(_) if self.lenient => Ok(None),
            Op::Assign(var) => Err(Error::assign_at_eval(var)),
            op => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
