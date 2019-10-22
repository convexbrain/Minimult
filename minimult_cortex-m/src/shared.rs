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
    /// Look a shared variable.
    /// * `f: F` - closure to refer the shared variable.
    /// * Blocks if the shared variable is touched by other channels.
    pub fn look<F>(&self, f: F)
    where F: FnOnce(&M)
    {
        let s = unsafe { self.s.as_mut().unwrap() };

        loop {
            if s.rw_cnt.incr_ifgt0() {
                break;
            }

            Minimult::wait(&s.rw_cnt, MTEventCond::GreaterThan(0));
        }

        f(&s.holder);

        s.rw_cnt.decr();
        Minimult::signal(&s.rw_cnt);
    }

    /// Touch a shared variable.
    /// * `f: F` - closure to mutably refer the shared variable.
    /// * Blocks if the shared variable is looked or touched by other channels.
    pub fn touch<F>(&self, f: F)
    where F: FnOnce(&mut M)
    {
        let s = unsafe { self.s.as_mut().unwrap() };

        loop {
            if s.rw_cnt.decr_if1() {
                break;
            }

            Minimult::wait(&s.rw_cnt, MTEventCond::Equal(1));
        }

        f(&mut s.holder);

        s.rw_cnt.incr();
        Minimult::signal(&s.rw_cnt);
    }
}
