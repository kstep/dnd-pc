use std::{fmt, marker::PhantomData};

use super::{Interpreter, eval_op};
use crate::expr::{Context, Error, Op, VarGroup, group::IterStack, ops::BlockIndex, stack::Stack};

// --- Evaluator (apply mode, mutable context) ---

pub struct Evaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    iter_stack: IterStack,
    ctx: &'a mut Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> Evaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: IterStack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>, Grp: VarGroup<Var = Var>>
    Interpreter<Var, i32, Grp> for Evaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32, Grp>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::AssignVar(var) => {
                self.ctx.assign(var, *self.stack.top()?)?;
                Ok(None)
            }
            Op::PushGroup(group) => {
                let var = group
                    .top_member(&self.iter_stack)
                    .ok_or(Error::GroupOutOfBounds)?;
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::AssignGroup(group) => {
                let var = group
                    .top_member(&self.iter_stack)
                    .ok_or(Error::GroupOutOfBounds)?;
                self.ctx.assign(var, *self.stack.top()?)?;
                Ok(None)
            }
            op => eval_op(&mut self.stack, &mut self.iter_stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- ReadOnlyEvaluator (eval mode, immutable context) ---

pub struct ReadOnlyEvaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    iter_stack: IterStack,
    ctx: &'a Ctx,
    lenient: bool,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> ReadOnlyEvaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: IterStack::new(),
            ctx,
            lenient: false,
            _var: PhantomData,
        }
    }

    pub fn lenient(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: IterStack::new(),
            ctx,
            lenient: true,
            _var: PhantomData,
        }
    }
}

impl<Var: Copy + fmt::Display, Ctx: Context<Var, i32>, Grp: VarGroup<Var = Var>>
    Interpreter<Var, i32, Grp> for ReadOnlyEvaluator<'_, Var, Ctx>
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32, Grp>) -> Result<Option<BlockIndex>, Error> {
        match op {
            Op::PushVar(var) => {
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            Op::AssignVar(_) if self.lenient => Ok(None),
            Op::AssignVar(var) => Err(Error::assign_at_eval(var)),
            Op::AssignGroup(_) if self.lenient => Ok(None),
            Op::AssignGroup(group) => {
                let var = group
                    .top_member(&self.iter_stack)
                    .ok_or(Error::GroupOutOfBounds)?;
                Err(Error::assign_at_eval(var))
            }
            Op::PushGroup(group) => {
                let var = group
                    .top_member(&self.iter_stack)
                    .ok_or(Error::GroupOutOfBounds)?;
                self.stack.push(self.ctx.resolve(var)?);
                Ok(None)
            }
            op => eval_op(&mut self.stack, &mut self.iter_stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
