use std::{fmt, marker::PhantomData};

use crate::expr::{
    Context, Error, Op, VarGroup,
    group::IterStack,
    interpret::{Interpreter, eval_op, handle_context_op, handle_push_op, resolve_group_var},
    ops::BlockIndex,
    stack::Stack,
};

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
        match handle_context_op(op, &mut self.stack, &self.iter_stack, self.ctx)? {
            None => Ok(None),
            Some(op) => eval_op(&mut self.stack, &mut self.iter_stack, op),
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
        let Some(op) = handle_push_op(op, &mut self.stack, &self.iter_stack, self.ctx)? else {
            return Ok(None);
        };
        match op {
            Op::AssignVar(_) | Op::AssignGroup(_) if self.lenient => Ok(None),
            Op::AssignVar(var) => Err(Error::assign_at_eval(var)),
            Op::AssignGroup(group) => {
                let var = resolve_group_var(group, &self.iter_stack)?;
                Err(Error::assign_at_eval(var))
            }
            op => eval_op(&mut self.stack, &mut self.iter_stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
