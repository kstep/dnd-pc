use std::{fmt, marker::PhantomData};

use crate::expr::{
    Context, Error, Op, ResolveGroup, VarGroup,
    interpret::{
        CursorStack, Interpreter, cursor_var, eval_op, handle_context_op, handle_loop_op,
        handle_push_op,
    },
    ops::BlockIndex,
    stack::Stack,
};

// --- Evaluator (apply mode, mutable context) ---

pub struct Evaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    iter_stack: CursorStack<Var>,
    ctx: &'a mut Ctx,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> Evaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a mut Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: CursorStack::new(),
            ctx,
            _var: PhantomData,
        }
    }
}

impl<Var, Ctx, Grp> Interpreter<Var, i32, Grp> for Evaluator<'_, Var, Ctx>
where
    Var: Copy + fmt::Display,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32> + ResolveGroup<Grp>,
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32, Grp>) -> Result<Option<BlockIndex>, Error> {
        match handle_context_op(op, &mut self.stack, &mut self.iter_stack, self.ctx)? {
            None => Ok(None),
            Some(op) => eval_op(&mut self.stack, op),
        }
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}

// --- ReadOnlyEvaluator (eval mode, immutable context) ---

pub struct ReadOnlyEvaluator<'a, Var, Ctx> {
    stack: Stack<i32>,
    iter_stack: CursorStack<Var>,
    ctx: &'a Ctx,
    lenient: bool,
    _var: PhantomData<Var>,
}

impl<'a, Var, Ctx> ReadOnlyEvaluator<'a, Var, Ctx> {
    pub fn new(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: CursorStack::new(),
            ctx,
            lenient: false,
            _var: PhantomData,
        }
    }

    pub fn lenient(ctx: &'a Ctx) -> Self {
        Self {
            stack: Stack::new(),
            iter_stack: CursorStack::new(),
            ctx,
            lenient: true,
            _var: PhantomData,
        }
    }
}

impl<Var, Ctx, Grp> Interpreter<Var, i32, Grp> for ReadOnlyEvaluator<'_, Var, Ctx>
where
    Var: Copy + fmt::Display,
    Grp: VarGroup<Var = Var>,
    Ctx: Context<Var, i32> + ResolveGroup<Grp>,
{
    type Output = i32;

    fn exec(&mut self, op: Op<Var, i32, Grp>) -> Result<Option<BlockIndex>, Error> {
        let Some(op) = handle_push_op(op, &mut self.stack, &self.iter_stack, self.ctx)? else {
            return Ok(None);
        };
        let op = match op {
            Op::AssignVar(_) | Op::AssignGroup(_, _) if self.lenient => return Ok(None),
            Op::AssignVar(var) => return Err(Error::assign_at_eval(var)),
            Op::AssignGroup(_grp, col) => {
                let var = cursor_var(&self.iter_stack, col)?;
                return Err(Error::assign_at_eval(var));
            }
            other => other,
        };
        let Some(op) = handle_loop_op(op, &mut self.stack, &mut self.iter_stack, self.ctx)? else {
            return Ok(None);
        };
        eval_op(&mut self.stack, op)
    }

    fn finish(self) -> Result<i32, Error> {
        self.stack.result()
    }
}
