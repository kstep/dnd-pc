use std::{fmt, marker::PhantomData, str::FromStr};

use serde::{Deserialize, Serialize};

/// Loop iteration index: `(real_group_index, sequential_position)`.
/// The real index maps directly to the group's member. The sequential
/// position counts 0, 1, 2, … across masked iterations and is used by
/// companion groups (`@ARG`) that provide one value per loop step.
pub type IterIndex = (usize, usize);

/// Trait for variable groups used in loop iteration.
/// Maps a group + index to a concrete variable.
pub trait VarGroup {
    type Var;

    /// Get the variable at the given index, or `None` if the group is
    /// exhausted.
    fn member(&self, index: usize) -> Option<Self::Var>;

    /// Find the index of a member by short name (e.g. "ACID" for Resist group).
    /// Returns `None` by default — override for groups with named members.
    fn member_by_name(&self, _name: &str) -> Option<usize> {
        None
    }

    /// Companion groups (like `@ARG`) are indexed by sequential loop
    /// position rather than the real group index. Override to return `true`
    /// for such groups.
    fn is_companion(&self) -> bool {
        false
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
    pub fn real_index(&self, position: usize) -> Option<usize> {
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
    pub fn next_real_index(&self, current: usize) -> Option<usize> {
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
        self.real_index(position)
            .and_then(|idx| self.inner.member(idx))
    }

    /// Iterate over real group indices of all members in this (sub)group.
    pub fn real_indices(&self) -> impl Iterator<Item = usize> + '_
    where
        Grp: VarGroup,
    {
        (0..).map_while(|pos| {
            let real_idx = self.real_index(pos)?;
            self.inner.member(real_idx)?;
            Some(real_idx)
        })
    }

    /// Initialize loop iteration. Pushes `(real_index, 0)` to `iter_stack`.
    /// Returns `true` if the group is non-empty (loop should enter body).
    pub fn init_loop(&self, iter_stack: &mut Vec<IterIndex>) -> bool
    where
        Grp: VarGroup,
    {
        if let Some(real_idx) = self.real_index(0)
            && self.inner.member(real_idx).is_some()
        {
            iter_stack.push((real_idx, 0));
            true
        } else {
            false
        }
    }

    /// Advance loop to the next member. Returns `true` if more items remain.
    /// When exhausted, pops `iter_stack` and returns `false`.
    pub fn advance_loop(&self, iter_stack: &mut Vec<IterIndex>) -> bool
    where
        Grp: VarGroup,
    {
        if let Some(&(current, seq)) = iter_stack.last()
            && let Some(next_idx) = self.next_real_index(current)
            && self.inner.member(next_idx).is_some()
        {
            *iter_stack.last_mut().unwrap() = (next_idx, seq + 1);
            true
        } else {
            iter_stack.pop();
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
                    && let Some(var) = self.inner.member(bit as usize)
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

    fn member(&self, _index: usize) -> Option<Var> {
        None
    }
}
