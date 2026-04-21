use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};

use crate::expr::stack::Stack;

/// Loop iteration state: carries both the real group index and the
/// sequential iteration number.  Each `VarGroup` decides which field
/// to use — most groups use `index` (the real position in the full
/// group), while companion groups like `@ARG` use `iter_no` (the
/// sequential loop counter).
#[derive(Debug, Copy, Clone, Default)]
pub struct IterIndex {
    /// Sequential iteration number (0, 1, 2, …).
    pub iter_no: usize,
    /// Real index into the group's member array.
    pub index: usize,
}

impl From<usize> for IterIndex {
    fn from(index: usize) -> Self {
        Self {
            iter_no: index,
            index,
        }
    }
}

/// Iteration stack used by expression evaluators.
pub type IterStack = Stack<IterIndex>;

/// Trait for variable groups used in loop iteration.
/// Maps a group + index to a concrete variable.
pub trait VarGroup {
    type Var;

    /// Get the variable at the given iteration index.
    ///
    /// Most groups use `index.index` (the real group position).
    /// Companion groups (like `@ARG`) use `index.iter_no` (the
    /// sequential loop counter).  Returns `None` if the group is
    /// exhausted.
    fn member(&self, index: IterIndex) -> Option<Self::Var>;

    /// Resolve the current (top-of-stack) iteration to a concrete variable.
    /// Returns `None` if the loop stack is empty or the group is exhausted.
    fn top_member(&self, iter_stack: &IterStack) -> Option<Self::Var> {
        let idx = iter_stack.top().ok().copied()?;
        self.member(idx)
    }

    /// Find the index of a member by short name (e.g. "ACID" for Resist group).
    /// Returns `None` by default — override for groups with named members.
    fn member_by_name(&self, _name: &str) -> Option<usize> {
        None
    }
}

/// A group with an optional bitmask to select a subset of members.
/// `mask == u32::MAX` means all members. Otherwise, only bits set in `mask`
/// are iterated, and `member(i)` returns the i-th set bit's element.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VarSubgroup<Grp> {
    pub inner: Grp,
    pub mask: u32,
}

impl<Grp> From<Grp> for VarSubgroup<Grp> {
    fn from(inner: Grp) -> Self {
        Self {
            inner,
            mask: u32::MAX,
        }
    }
}

impl<Grp> VarSubgroup<Grp> {
    pub fn masked(inner: Grp, mask: u32) -> Self {
        Self { inner, mask }
    }

    /// Get the real group index for the i-th set bit in mask.
    fn real_index(&self, position: usize) -> Option<usize> {
        let mut remaining = position;
        for bit in 0..32u32 {
            if self.mask & (1 << bit) != 0 {
                if remaining == 0 {
                    return Some(bit as usize);
                }
                remaining -= 1;
            }
        }
        None
    }

    /// Advance from the current real group index to the next set bit.
    fn next_real_index(&self, current: usize) -> Option<usize> {
        for bit in (current as u32 + 1)..32 {
            if self.mask & (1 << bit) != 0 {
                return Some(bit as usize);
            }
        }
        None
    }

    /// Get the i-th member of this (sub)group.
    pub fn member(&self, position: usize) -> Option<Grp::Var>
    where
        Grp: VarGroup,
    {
        let index = self.real_index(position)?;
        self.inner.member(IterIndex {
            iter_no: position,
            index,
        })
    }

    /// Iterate over all members of this (sub)group, yielding `IterIndex`
    /// with both the sequential iteration number and the real group index.
    pub fn iter_indices(&self) -> impl Iterator<Item = IterIndex> + '_
    where
        Grp: VarGroup,
    {
        (0..).map_while(|iter_no| {
            let index = self.real_index(iter_no)?;
            self.inner.member(index.into())?;
            Some(IterIndex { iter_no, index })
        })
    }

    /// Iterate the concrete member values (`Var`s) permitted by this
    /// subgroup's mask, in iteration order.
    pub fn members(&self) -> impl Iterator<Item = Grp::Var> + '_
    where
        Grp: VarGroup,
    {
        (0..).map_while(|iter_no| {
            let index = self.real_index(iter_no)?;
            self.inner.member(IterIndex { iter_no, index })
        })
    }

    /// Initialize loop iteration. Pushes the first `IterIndex` onto the
    /// stack.  Returns `true` if the group is non-empty (loop should
    /// enter body).
    pub(crate) fn init_loop(&self, iter_stack: &mut IterStack) -> bool
    where
        Grp: VarGroup,
    {
        if let Some(real_idx) = self.real_index(0)
            && self.inner.member(real_idx.into()).is_some()
        {
            iter_stack.push(IterIndex {
                iter_no: 0,
                index: real_idx,
            });
            true
        } else {
            false
        }
    }

    /// Advance loop to the next member. Returns `true` if more items remain.
    /// When exhausted, pops `iter_stack` and returns `false`.
    pub(crate) fn advance_loop(&self, iter_stack: &mut IterStack) -> bool
    where
        Grp: VarGroup,
    {
        if let Ok(current) = iter_stack.top()
            && let Some(next_idx) = self.next_real_index(current.index)
            && self.inner.member(next_idx.into()).is_some()
        {
            let iter_no = current.iter_no + 1;
            *iter_stack.top_mut().unwrap() = IterIndex {
                iter_no,
                index: next_idx,
            };
            true
        } else {
            let _ = iter_stack.pop();
            false
        }
    }

    /// Format as `GROUP` or `GROUP(elem, elem, ...)` when masked.
    pub fn display_with(&self, f: &mut fmt::Formatter) -> fmt::Result
    where
        Grp: Copy + VarGroup + fmt::Display,
        Grp::Var: fmt::Display,
    {
        write!(f, "{}", self.inner)?;
        if self.mask != u32::MAX {
            write!(f, "(")?;
            let mut first = true;
            for bit in 0..32u32 {
                if self.mask & (1 << bit) != 0
                    && let Some(var) = self.inner.member((bit as usize).into())
                {
                    if !first {
                        write!(f, ", ")?;
                    }
                    write!(f, "{var}")?;
                    first = false;
                }
            }
            write!(f, ")")?;
        }
        Ok(())
    }
}

impl<Grp> fmt::Display for VarSubgroup<Grp>
where
    Grp: Copy + VarGroup + fmt::Display,
    Grp::Var: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.display_with(f)
    }
}

/// Default no-op group type for expressions without loop support.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoGroup<Var = ()>(PhantomData<Var>);

impl<Var> Default for NoGroup<Var> {
    fn default() -> Self {
        Self(PhantomData)
    }
}

impl<Var> fmt::Display for NoGroup<Var> {
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

impl<Var> FromStr for NoGroup<Var> {
    type Err = &'static str;

    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        Err("no groups supported")
    }
}

impl<Var> VarGroup for NoGroup<Var> {
    type Var = Var;

    fn member(&self, _index: IterIndex) -> Option<Var> {
        None
    }
}
