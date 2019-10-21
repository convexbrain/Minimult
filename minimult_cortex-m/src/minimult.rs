use crate::{MTTaskId, MTTaskPri};
use crate::msgq::MTMsgQueue;
use crate::memory::{MTMemBlk, MTAlloc};
use crate::kernel::{mtkernel_create, mtkernel_get_ref, mtkernel_get_mut};

/// Multitasking API
pub struct Minimult<'a>
{
    alloc: MTAlloc<'a>
}

impl<'a> Minimult<'a>
{
    // ----- ----- Main context ----- ----- //

    /// Reserves a memory block to be used by `Minimult` instance.
    /// * Any type `B` specifies a size of the memory block. Typically use `[u8; N]` for `N` bytes.
    /// * Returns the reserved memory block.
    pub const fn mem<B>() -> MTMemBlk<B>
    {
        MTMemBlk::new()
    }

    /// Creates `Minimult` instance.
    /// * `mem` - reserved memory block.
    /// * `num_tasks` - number of tasks.
    /// * Returns the created instance.
    /// * (`num_tasks` * (32 + 6)) bytes of the memory block is consumed.
    pub fn new<B>(mem: &mut MTMemBlk<B>, num_tasks: MTTaskId) -> Minimult
    {
        let mut alloc = MTAlloc::new(mem);

        mtkernel_create(alloc.array(num_tasks), alloc.array(num_tasks));

        Minimult {
            alloc
        }
    }

    /// Creates a message queue.
    /// * `M` - type of the message element.
    /// * `len` - length of the message queue array.
    /// * Returns the created message queue.
    /// * (`len` * (size of `Option<M>`)) bytes of the memory block is consumed.
    pub fn msgq<M>(&mut self, len: usize) -> MTMsgQueue<'a, M>
    {
        let mem = self.alloc.array(len);

        MTMsgQueue::new(mem)
    }

    /// Registers a closure as a task.
    /// * `tid` - task identifier. `0` to `num_tasks - 1`.
    /// * `pri` - task priority. The lower value is the higher priority.
    /// * `stack_len` - length of a stack used by the task.
    /// * `task: T` - task closure.
    /// * (`stack_len` * size of `usize`) bytes of the memory block is consumed.
    pub fn register<T>(&mut self, tid: MTTaskId, pri: MTTaskPri, stack_len: usize, task: T)
    where T: FnOnce() + Send + 'a
    {
        let tm = mtkernel_get_mut().unwrap();

        let stack = self.alloc.array(stack_len);
        
        tm.register_once(tid, pri, stack, task);
    }

    /// Runs into a loop to dispatch the registered tasks.
    /// * Never returns.
    pub fn run(self) -> !
    {
        let tm = mtkernel_get_mut().unwrap();

        tm.run()
    }

    // ----- ----- Task context ----- ----- //

    /// Brings a current running task into an idle state.
    pub fn idle()
    {
        if let Some(tm) = mtkernel_get_mut() {
            tm.idle();
        }
    }

    pub(crate) fn wait()
    {
        if let Some(tm) = mtkernel_get_mut() {
            tm.wait();
        }
    }

    pub(crate) fn signal(tid: MTTaskId)
    {
        if let Some(tm) = mtkernel_get_mut() {
            tm.signal(tid);
        }
    }

    // ----- ----- Task and Interrupt context ----- ----- //

    /// Makes a service call to request dispatching.
    pub fn dispatch()
    {
        if let Some(tm) = mtkernel_get_ref() {
            tm.dispatch();
        }
    }

    /// Wakes up a task in an idle state.
    /// * `tid` - task identifier. `0` to `num_tasks - 1`.
    pub fn kick(tid: MTTaskId)
    {
        if let Some(tm) = mtkernel_get_mut() {
            tm.kick(tid);
        }
    }

    /// Gets task identifier of a current running task if any.
    /// * Returns task identifier in `Option`.
    pub fn curr_tid() -> Option<MTTaskId>
    {
        if let Some(tm) = mtkernel_get_ref() {
            tm.curr_tid()
        }
        else {
            None
        }
    }
}

impl Drop for Minimult<'_>
{
    fn drop(&mut self)
    {
        panic!("Minimult dropped without a run");
    }
}
