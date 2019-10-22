use core::marker::PhantomData;

use crate::minimult::Minimult;
use crate::memory::MTRawArray;
use crate::kernel::{MTEvent, MTEventCond};

//

fn wrap_inc(x: usize, bound: usize) -> usize
{
    let y = x + 1;
    if y < bound {y} else {0}
}

//

/// Message queue for task-to-task communication
pub struct MTMsgQueue<'a, M>
{
    mem: MTRawArray<Option<M>>,
    wr_idx: usize,
    rd_idx: usize,
    msg_cnt: MTEvent,
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
            msg_cnt: MTEvent::new(0),
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

        q.mem.len() - q.msg_cnt.cnt()
    }

    /// Sends a message.
    /// * `msg` - the message to be sent.
    /// * Blocks if there is no vacant message entry.
    pub fn send(&mut self, msg: M)
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        loop {
            if q.msg_cnt.cnt() < q.mem.len() {
                break;
            }

            Minimult::wait(&q.msg_cnt, MTEventCond::LessThan(q.mem.len()));
        }

        let curr_wr_idx = q.wr_idx;
        let next_wr_idx = wrap_inc(curr_wr_idx, q.mem.len());

        q.mem.write_volatile(curr_wr_idx, Some(msg));

        q.wr_idx = next_wr_idx; // NOTE: atomic access might be necessary

        q.msg_cnt.incr();
        Minimult::signal(&q.msg_cnt);
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

        q.msg_cnt.cnt()
    }

    /// Receives a message.
    /// * `f: F` - closure to refer the received message.
    /// * Blocks if there is no available message entry.
    pub fn receive<F>(&mut self, f: F)
    where F: FnOnce(&M)
    {
        let q = unsafe { self.q.as_mut().unwrap() };

        loop {
            if q.msg_cnt.cnt() > 0 {
                break;
            }

            Minimult::wait(&q.msg_cnt, MTEventCond::GreaterThan(0));
        }

        let curr_rd_idx = q.rd_idx;
        let next_rd_idx = wrap_inc(curr_rd_idx, q.mem.len());

        let ptr = q.mem.refer(curr_rd_idx);

        f(ptr.as_ref().unwrap());
        ptr.take().unwrap();

        q.rd_idx = next_rd_idx; // NOTE: atomic access might be necessary

        q.msg_cnt.decr();
        Minimult::signal(&q.msg_cnt);
    }
} 
