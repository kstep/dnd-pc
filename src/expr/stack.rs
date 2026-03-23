use std::ops::Deref;

use crate::expr::Error;

pub(super) struct Stack<T>(Vec<T>);

impl<T> Deref for Stack<T> {
    type Target = [T];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Stack<T> {
    const DEFAULT_CAPACITY: usize = 16;

    pub fn new() -> Self {
        Self(Vec::with_capacity(Self::DEFAULT_CAPACITY))
    }

    pub fn push(&mut self, val: T) {
        self.0.push(val);
    }

    pub fn pop(&mut self) -> Result<T, Error> {
        self.0.pop().ok_or(Error::StackUnderflow)
    }

    pub fn pop2(&mut self) -> Result<(T, T), Error> {
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b))
    }

    pub fn pop3(&mut self) -> Result<(T, T, T), Error> {
        let c = self.pop()?;
        let b = self.pop()?;
        let a = self.pop()?;
        Ok((a, b, c))
    }

    pub fn result(mut self) -> Result<T, Error> {
        self.pop()
    }

    /// Pop `count` items, apply `f` to the slice, push the result.
    pub fn pop_n_reduce(
        &mut self,
        count: usize,
        f: impl FnOnce(&mut [T]) -> T,
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

    pub fn top(&self) -> Result<&T, Error> {
        self.0.last().ok_or(Error::StackUnderflow)
    }
}

impl<T> IntoIterator for Stack<T> {
    type IntoIter = std::vec::IntoIter<T>;
    type Item = T;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}
