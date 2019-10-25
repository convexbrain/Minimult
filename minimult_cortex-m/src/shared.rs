use core::marker::PhantomData;

use crate::minimult::Minimult;
use crate::kernel::{MTEvent, MTEventCond};

/// Shared variable among tasks
pub struct MTShared<'a, M>
{
    holder: M,
    rw_cnt: MTEvent,
    phantom: PhantomData<&'a ()>
}

impl<'a, M> MTShared<'a, M>
{
    pub(crate) fn new(holder: M) -> MTShared<'a, M> // NOTE: lifetime safety correctness
    {
        MTShared {
            holder,
            rw_cnt: MTEvent::new(1),
            phantom: PhantomData
        }
    }

    /// Gets a shared variable access channel.
    /// * Returns the shared access channel.
    pub fn ch<'s>(&'s self) -> MTSharedCh<'a, 's, M>
    {
        MTSharedCh {
            s: (self as *const Self) as *mut Self, // NOTE: mutability conversion
            phantom: PhantomData
        }
    }
}

//

/// Shared variable access channel
pub struct MTSharedCh<'a, 's, M>
{
    s: *mut MTShared<'a, M>,
    phantom: PhantomData<&'s ()>
}

unsafe impl<M: Send> Send for MTSharedCh<'_, '_, M> {}

impl<M> MTSharedCh<'_, '_, M>
{
    /// Makes an immutable access to a shared variable.
    /// * Returns a `Deref`-able wrapper of the shared variable.
    /// * Blocks if the shared variable is `touch`ed by other channels.
    pub fn look<'c>(&'c self) -> MTSharedLook<'c, M>
    {
        loop {
            if let Some(v) = self.try_look() {
                return v;
            }
            else {
                let s = unsafe { self.s.as_mut().unwrap() };
                Minimult::wait(&s.rw_cnt, MTEventCond::GreaterThan(0));
            }
        }
    }

    /// Tries to make an immutable access to a shared variable.
    /// * Returns a `Deref`-able wrapper of the shared variable in `Option`.
    /// * Gets `None` if the shared variable is `touch`ed by other channels.
    pub fn try_look<'c>(&'c self) -> Option<MTSharedLook<'c, M>>
    {
        let s = unsafe { self.s.as_mut().unwrap() };

        if s.rw_cnt.incr_ifgt0() {
            Some(MTSharedLook {
                holder: &s.holder,
                rw_cnt: &mut s.rw_cnt
            })
        }
        else {
            None
        }
    }

    /// Makes a mutable access to a shared variable.
    /// * Returns a `DerefMut`-able wrapper of the shared variable.
    /// * Blocks if the shared variable is `look`ed or `touch`ed by other channels.
    pub fn touch<'c>(&'c self) -> MTSharedTouch<'c, M>
    {
        loop {
            if let Some(v) = self.try_touch() {
                return v;
            }
            else {
                let s = unsafe { self.s.as_mut().unwrap() };
                Minimult::wait(&s.rw_cnt, MTEventCond::Equal(1));
            }
        }
    }

    /// Tries to make a mutable access to a shared variable.
    /// * Returns a `DerefMut`-able wrapper of the shared variable in `Option`.
    /// * Gets `None` if the shared variable is `look`ed or `touch`ed by other channels.
    pub fn try_touch<'c>(&'c self) -> Option<MTSharedTouch<'c, M>>
    {
        let s = unsafe { self.s.as_mut().unwrap() };

        if s.rw_cnt.decr_if1() {
            Some(MTSharedTouch {
                holder: &mut s.holder,
                rw_cnt: &mut s.rw_cnt
            })
        }
        else {
            None
        }
    }
}

//

/// Shared variable's immutable access scope wrapper
pub struct MTSharedLook<'c, M>
{
    holder: &'c M,
    rw_cnt: &'c mut MTEvent,
}

impl<M> core::ops::Deref for MTSharedLook<'_, M>
{
    type Target = M;

    fn deref(&self) -> &Self::Target
    {
        self.holder
    }
}

impl<M> Drop for MTSharedLook<'_, M>
{
    fn drop(&mut self)
    {
        self.rw_cnt.decr();
        Minimult::signal(&self.rw_cnt);
    }
}

//

/// Shared variable's mutable access scope wrapper
pub struct MTSharedTouch<'c, M>
{
    holder: &'c mut M,
    rw_cnt: &'c mut MTEvent,
}

impl<M> core::ops::Deref for MTSharedTouch<'_, M>
{
    type Target = M;

    fn deref(&self) -> &Self::Target
    {
        self.holder
    }
}

impl<M> core::ops::DerefMut for MTSharedTouch<'_, M>
{
    fn deref_mut(&mut self) -> &mut Self::Target
    {
        self.holder
    }
}

impl<M> Drop for MTSharedTouch<'_, M>
{
    fn drop(&mut self)
    {
        self.rw_cnt.incr();
        Minimult::signal(&self.rw_cnt);
    }
}
