use core::marker::PhantomData;

use crate::MTTaskId;
use crate::minimult::Minimult;
use crate::memory::MTRawArray;

//

fn wrap_inc(x: usize, bound: usize) -> usize
{
    let y = x + 1;
    if y < bound {y} else {0}
}

fn wrap_diff(x: usize, y: usize, bound: usize) -> usize
{
    if x >= y {
        x - y
    }
    else {
        x + (bound - y)
    }
}

//

/// Message queue for task-to-task communication
pub struct MTMsgQueue<'a, M>
{
    mem: MTRawArray<Option<M>>,
    wr_idx: usize,
    rd_idx: usize,
    wr_tid: Option<MTTaskId>,
    rd_tid: Option<MTTaskId>,
    phantom: PhantomData<&'a ()>
}

impl<'a, M> MTMsgQueue<'a, M>
{
    pub(crate) fn new(mem: MTRawArray<Option<M>>) -> MTMsgQueue<'a, M> // NOTE: lifetime safety correctness
    {
        MTMsgQueue {
            mem,
            wr_idx: 0,
            rd_idx: 0,
            wr_tid: None,
            rd_tid: None,
            phantom: PhantomData
        }
    }

    /// Gets sending and receving channels.
    /// * Returns a tuple of the sender and receiver pair.
    pub fn ch<'q>(&'q mut self) -> (MTMsgSender<'a, 'q, M>, MTMsgReceiver<'a, 'q, M>)
    {
        (
            MTMsgSender {
                q: self,
                phantom: PhantomData
            },
            MTMsgReceiver {
                q: self,
                phantom: PhantomData
            }
        )
    }
}

//

/// Message sending channel
pub struct MTMsgSender<'a, 'q, M>
{
    q: *mut MTMsgQueue<'a, M>,
    phantom: PhantomData<&'q ()>
}

unsafe impl<M: Send> Send for MTMsgSender<'_, '_, M> {}

impl<M> MTMsgSender<'_, '_, M>
{
    /// Gets if there is a vacant message entry.
    /// * Returns the number of vacant message entries.
    pub fn vacant(&self) -> usize
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        q.wr_tid = Minimult::curr_tid();

        wrap_diff(q.rd_idx, wrap_inc(q.wr_idx, q.mem.len()), q.mem.len())
    }

    /// Sends a message.
    /// * `msg` - the message to be sent.
    /// * Blocks if there is no vacant message entry.
    pub fn send(&mut self, msg: M)
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        q.wr_tid = Minimult::curr_tid();

        let curr_wr_idx = q.wr_idx;
        let next_wr_idx = wrap_inc(curr_wr_idx, q.mem.len());

        loop {
            if next_wr_idx == q.rd_idx {
                Minimult::wait();
            }
            else {
                break;
            }
        }

        q.mem.write_volatile(curr_wr_idx, Some(msg));

        q.wr_idx = next_wr_idx; // NOTE: atomic access might be necessary

        if let Some(rd_tid) = q.rd_tid {
            Minimult::signal(rd_tid);
        }
    }
}

//

/// Message receiving channel
pub struct MTMsgReceiver<'a, 'q, M>
{
    q: *mut MTMsgQueue<'a, M>,
    phantom: PhantomData<&'q ()>
}

unsafe impl<M: Send> Send for MTMsgReceiver<'_, '_, M> {}

impl<M> MTMsgReceiver<'_, '_, M>
{
    /// Gets if there is an available message entry.
    /// * Returns the number of available message entries.
    pub fn available(&self) -> usize
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        q.rd_tid = Minimult::curr_tid();

        wrap_diff(q.wr_idx, q.rd_idx, q.mem.len())
    }

    /// Receives a message.
    /// * `f: F` - closure to refer the received message.
    /// * Blocks if there is no available message entry.
    pub fn receive<F>(&mut self, f: F)
    where F: FnOnce(&M)
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        q.rd_tid = Minimult::curr_tid();

        let curr_rd_idx = q.rd_idx;
        let next_rd_idx = wrap_inc(curr_rd_idx, q.mem.len());

        loop {
            if curr_rd_idx == q.wr_idx {
                Minimult::wait();
            }
            else {
                break;
            }
        }

        let ptr = q.mem.refer(curr_rd_idx);

        f(ptr.as_ref().unwrap());
        ptr.take().unwrap();

        q.rd_idx = next_rd_idx; // NOTE: atomic access might be necessary

        if let Some(wr_tid) = q.wr_tid {
            Minimult::signal(wr_tid);
        }
    }
} 
