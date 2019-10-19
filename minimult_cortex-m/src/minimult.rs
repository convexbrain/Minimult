// TODO: module
// TODO: clarify TaskMgr/Minimult role separation

/* Type parameter rule
 * A: method-level general variable
 * F: method-level general closure
 * V: struct-level general variable
 * T: task closure
 * I: index
 * K: key
 * M: message
 * B: memory block
 */

use cortex_m::peripheral::SCB;

use core::mem::{MaybeUninit, size_of, align_of, transmute};
use core::marker::PhantomData;


/// Task identifier
pub type MTTaskId = u16;

/// Task priority
pub type MTTaskPri = u8;

//

fn align_up<A>(x: usize) -> usize
{
    let align = align_of::<A>();
    let y = (x + align - 1) / align;
    let y = y * align;
    y
}

fn align_down<A>(x: usize) -> usize
{
    let align = align_of::<A>();
    let y = x / align;
    let y = y * align;
    y
}

extern "C" {
    fn minimult_ex_cntup(exc: &mut usize);
}

fn ex_cntup(exc: &mut usize)
{
    unsafe {
        minimult_ex_cntup(exc);
    }
}


//

struct RefFnOnce
{
    data: *const u8,
    vtbl: *const usize
}

fn inf_loop() -> !
{
    let tm = unsafe { O_TASKMGR.as_mut().unwrap() };
    tm.none();

    loop {}
}

//

struct MTBHeapDList<I, K>
{
    array: MTRawArray<Option<(I, K)>>,
    n_bheap: I,
    n_flist: I
}

impl<I, K> MTBHeapDList<I, K>
where I: num_integer::Integer + Into<usize> + Copy, K: Ord
{
    fn new(array: MTRawArray<Option<(I, K)>>) -> MTBHeapDList<I, K>
    {
        MTBHeapDList {
            array,
            n_bheap: I::zero(),
            n_flist: I::zero()
        }
    }

    fn replace(&mut self, pos0: I, pos1: I)
    {
        if pos0 != pos1 {
            let tmp0 = self.array.refer(pos0).take();
            let tmp1 = self.array.refer(pos1).take();
            self.array.write(pos0, tmp1);
            self.array.write(pos1, tmp0);
        }
    }

    fn up_bheap(&mut self)
    {
        let two = I::one() + I::one();

        if self.n_bheap > I::zero() {
            let mut pos = self.n_bheap - I::one();

            while pos > I::zero() {
                let parent = (pos - I::one()) / two;

                let key_pos = &self.array.refer(pos).as_ref().unwrap().1;
                let key_parent = &self.array.refer(parent).as_ref().unwrap().1;

                if key_pos >= key_parent {
                    break;
                }

                self.replace(pos, parent);
                pos = parent;
            }
        }
    }

    fn down_bheap(&mut self)
    {
        let two = I::one() + I::one();

        let mut pos = I::zero();

        while pos < self.n_bheap / two {
            let child0 = (pos * two) + I::one();
            let child1 = (pos * two) + two;

            let key_pos = &self.array.refer(pos).as_ref().unwrap().1;
            let key_child0 = &self.array.refer(child0).as_ref().unwrap().1;

            let (child, key_child) = if child1 < self.n_bheap {
                let key_child1 = &self.array.refer(child1).as_ref().unwrap().1;

                if key_child0 <= key_child1 {
                    (child0, key_child0)
                }
                else {
                    (child1, key_child1)
                }
            }
            else {
                (child0, key_child0)
            };

            if key_pos < key_child {
                break;
            }

            self.replace(pos, child);
            pos = child;
        }
    }

    fn add_bheap(&mut self, id: I, key: K)
    {
        // add flist tail
        let pos = self.n_bheap + self.n_flist;
        self.array.write(pos, Some((id, key)));
        self.n_flist = self.n_flist + I::one();

        // flist tail => bheap
        self.flist_to_bheap(pos);
    }

    fn flist_to_bheap(&mut self, pos: I)
    {
        assert!(pos >= self.n_bheap);
        assert!(pos < self.n_bheap + self.n_flist);

        // replace flist pos <=> flist head
        self.replace(pos, self.n_bheap);

        // flist head <=> bheap tail
        self.n_flist = self.n_flist - I::one();
        self.n_bheap = self.n_bheap + I::one();

        self.up_bheap();
    }

    fn bheap_h_to_flist_h(&mut self)
    {
        assert!(self.n_bheap > I::zero());
        
        // replace bheap head <=> bheap tail
        let pos1 = self.n_bheap - I::one();
        self.replace(I::zero(), pos1);

        // bheap tail <=> flist head
        self.n_flist = self.n_flist + I::one();
        self.n_bheap = self.n_bheap - I::one();

        self.down_bheap();
    }

    fn round_bheap_h(&mut self)
    {
        self.bheap_h_to_flist_h();

        self.flist_to_bheap(self.n_bheap);
    }

    fn remove_bheap_h(&mut self)
    {
        self.bheap_h_to_flist_h();

        // replace flist head <=> flist tail
        let pos1 = self.n_bheap + self.n_flist - I::one();
        self.replace(self.n_bheap, pos1);

        // remove flist tail
        self.array.write(pos1, None);
        self.n_flist = self.n_flist - I::one();
    }

    fn bheap_h(&self) -> Option<I>
    {
        if self.n_bheap > I::zero() {
            Some(self.array.refer(I::zero()).as_ref().unwrap().0)
        }
        else {
            None
        }
    }

    fn flist_scan<F>(&mut self, to_bheap: F)
    where F: Fn(I) -> bool
    {
        let pos_b = self.n_bheap;
        let pos_e = pos_b + self.n_flist;

        let mut pos = pos_b;
        while pos < pos_e {
            if to_bheap(self.array.refer(pos).as_ref().unwrap().0) {
                self.flist_to_bheap(pos);
            }
            pos = pos + I::one();
        }
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum MTState
{
    None,
    Idle,
    Ready,
    Waiting
}

struct MTTask
{
    sp_start: *mut usize,
    sp_end: *mut usize,
    //
    sp: *mut usize,
    kick_excnt: usize,
    wakeup_cnt: usize,
    signal_excnt: usize,
    wait_cnt: usize,
    state: MTState
}

struct MTTaskMgr
{
    tasks: MTRawArray<MTTask>,
    task_tree: MTBHeapDList<MTTaskId, MTTaskPri>,
    //
    sp_loops: *mut usize,
    tid: Option<MTTaskId>
}

impl MTTaskMgr
{
    fn task_current(&mut self) -> Option<&mut MTTask>
    {
        if let Some(curr_tid) = self.tid {
            Some(self.tasks.refer(curr_tid))
        }
        else {
            None
        }
    }

    // Main context

    fn new(tasks: MTRawArray<MTTask>, task_tree_array: MTRawArray<Option<(MTTaskId, MTTaskPri)>>) -> MTTaskMgr
    {
        for i in 0..tasks.len() {
            tasks.write(i,
                MTTask {
                    sp_start: core::ptr::null_mut(),
                    sp_end: core::ptr::null_mut(),
                    sp: core::ptr::null_mut(),
                    kick_excnt: 0,
                    wakeup_cnt: 0,
                    signal_excnt: 0,
                    wait_cnt: 0,
                    state: MTState::None
                }
            );
            task_tree_array.write(i, None);
        }

        MTTaskMgr {
            tasks,
            task_tree: MTBHeapDList::new(task_tree_array),
            sp_loops: core::ptr::null_mut(),
            tid: None
        }
    }

    fn setup_task_once(sp: *mut usize, data: *mut u8, call_once: usize)
    {
        // TODO: magic number

        unsafe {
            // r0
            sp.add(8 + 0).write_volatile(data as usize);
            
            // lr
            sp.add(8 + 5).write_volatile(inf_loop as usize);

            // ReturnAddress
            sp.add(8 + 6).write_volatile(call_once);

            // xPSR: set T-bit since Cortex-M has only Thumb instructions
            sp.add(8 + 7).write_volatile(0x01000000);
        }
    }

    fn register_once<T>(&mut self, tid: MTTaskId, pri: MTTaskPri, stack: MTRawArray<usize>, t: T)
    where T: FnOnce() + Send // unsafe lifetime
    {
        let task = self.tasks.refer(tid);

        assert_eq!(task.state, MTState::None); // TODO: better message

        let sp_start = stack.head();
        let sp_end = stack.tail();

        let sz = size_of::<T>();
        let rfo = unsafe { transmute::<&dyn FnOnce(), RefFnOnce>(&t) };

        let sp = sp_end as usize;
        let sp = align_down::<T>(sp - sz);
        let data = sp as *mut u8;
        let sp = align_down::<u64>(sp); // 8-byte align
        let sp = sp as *mut usize;
        let sp = unsafe { sp.sub(26 + 2/*margin*/) }; // TODO: magic number

        unsafe {
            core::intrinsics::copy(rfo.data, data, sz)
        }

        let vtbl = rfo.vtbl;
        let call_once = unsafe { vtbl.add(3).read() }; // TODO: magic number

        MTTaskMgr::setup_task_once(sp, data, call_once);

        assert!(sp >= sp_start); // TODO: better message
        assert!(sp <= sp_end); // TODO: better message

        task.sp_start = sp_start;
        task.sp_end = sp_end;
        task.sp = sp;
        task.state = MTState::Ready;

        self.task_tree.add_bheap(tid, pri);
    }

    // Interrupt context

    fn save_sp(&mut self, curr_sp: *mut usize) -> *mut usize
    {
        // check and save current sp

        if let Some(task) = self.task_current() {
            assert!(curr_sp >= task.sp_start); // TODO: better message
            assert!(curr_sp <= task.sp_end); // TODO: better message

            task.sp = curr_sp;
        }
        else {
            self.sp_loops = curr_sp;
        }

        self.sp_loops // use sp_loops until switching task
    }

    fn task_switch(&mut self) -> *mut usize
    {
        // clear service call request

        SCB::clear_pendsv();

        // change state

        if let Some(task) = self.task_current() {
            match task.state {
                MTState::None => {
                    self.task_tree.remove_bheap_h();
                }
                MTState::Idle => {
                    self.task_tree.bheap_h_to_flist_h();
                }
                MTState::Waiting => {
                    self.task_tree.bheap_h_to_flist_h();
                }
                _  => {}
            }
        }
        // scan to check if Idle/Wait to Ready

        let tasks = &self.tasks;

        self.task_tree.flist_scan(|tid| {
            let task = tasks.refer(tid);

            match task.state {
                MTState::Idle => {
                    if task.kick_excnt != task.wakeup_cnt {
                        task.wakeup_cnt = task.wakeup_cnt.wrapping_add(1);
                        task.state = MTState::Ready;
                        true
                    }
                    else {
                        false
                    }
                }
                MTState::Waiting => {
                    if task.signal_excnt != task.wait_cnt {
                        task.wait_cnt = task.signal_excnt;
                        task.state = MTState::Ready;
                        true
                    }
                    else {
                        false
                    }
                }
                _ => panic!() // TODO: better message
            }
        });

        // round robin

        if let Some(task) = self.task_current() {
            match task.state {
                MTState::Ready => {
                    self.task_tree.round_bheap_h();
                }
                _  => {}
            }
        }

        // find highest priority Ready task

        let (next_tid, next_sp) = if let Some(tid) = self.task_tree.bheap_h() {
            (Some(tid), self.tasks.refer(tid).sp)
        }
        else {
            (None, self.sp_loops)
        };

        self.tid = next_tid;

        next_sp
    }

    // Task context

    fn idle(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::Idle; // TODO: atomic access in case
        
        Minimult::dispatch();
    }

    fn none(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::None; // TODO: atomic access in case
        
        Minimult::dispatch();
    }

    fn wait(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::Waiting; // TODO: atomic access in case
        
        Minimult::dispatch();
    }

    fn signal(&mut self, tid: MTTaskId)
    {
        let task = self.tasks.refer(tid);

        ex_cntup(&mut task.signal_excnt);

        Minimult::dispatch();
    }

    // Task and Interrupt context

    fn kick(&mut self, tid: MTTaskId)
    {
        let task = self.tasks.refer(tid);

        ex_cntup(&mut task.kick_excnt);

        Minimult::dispatch();
    }
}

//

static mut O_TASKMGR: Option<MTTaskMgr> = None;
static mut LOOP_STARTED: bool = false;

#[no_mangle]
extern fn minimult_save_sp(curr_sp: *mut usize) -> *mut usize
{
    let tm = unsafe { O_TASKMGR.as_mut().unwrap() };
    tm.save_sp(curr_sp)
}

#[no_mangle]
extern fn minimult_task_switch() -> *mut usize
{
    let tm = unsafe { O_TASKMGR.as_mut().unwrap() };
    tm.task_switch()
}

//

/// Memory block used by `Minimult`
pub struct MTMemBlk<B>(MaybeUninit<B>);

impl<B> MTMemBlk<B>
{
    const fn new() -> MTMemBlk<B>
    {
        MTMemBlk(MaybeUninit::<B>::uninit())
    }

    fn size(&self) -> usize
    {
        size_of::<B>()
    }

    fn head(&mut self) -> usize
    {
        self.0.as_mut_ptr() as usize
    }
}

//

struct MTRawArray<V>
{
    head: *mut V,
    len: usize
}

impl<V> MTRawArray<V>
{
    fn refer<I>(&self, i: I) -> &mut V
    where I: Into<usize>
    {
        let i = i.into();
        assert!(i < self.len); // TODO: better message

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.as_mut().unwrap() }
    }

    fn write<I>(&self, i: I, v: V)
    where I: Into<usize>
    {
        let i = i.into();
        assert!(i < self.len); // TODO: better message

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.write(v); }
    }

    fn write_volatile<I>(&self, i: I, v: V)
    where I: Into<usize>
    {
        let i = i.into();
        assert!(i < self.len); // TODO: better message

        let ptr = self.head;
        let ptr = unsafe { ptr.add(i) };

        unsafe { ptr.write_volatile(v); }
    }

    fn head(&self) -> *mut V
    {
        self.head
    }

    fn len(&self) -> usize
    {
        self.len
    }

    fn tail(&self) -> *mut V
    {
        let ptr = self.head;
        let ptr = unsafe { ptr.add(self.len) };
        ptr
    }
}

//

struct MTAlloc<'a>
{
    cur_pos: usize,
    end_cap: usize,
    phantom: PhantomData<&'a ()>
}

impl<'a> MTAlloc<'a>
{
    fn new<'b, B>(mem: &'b mut MTMemBlk<B>) -> MTAlloc<'b>
    {
        MTAlloc {
            cur_pos: mem.head(),
            end_cap: mem.head() + mem.size(),
            phantom: PhantomData
        }
    }

    fn array<V, A>(&mut self, len: A) -> MTRawArray<V>
    where A: Into<usize>
    {
        let len = len.into();
        let size = size_of::<V>() * len;

        let p = align_up::<V>(self.cur_pos);
        let e = p + size;

        assert!(e <= self.end_cap); // TODO: better message

        self.cur_pos = e;

        MTRawArray {
            head: p as *mut V,
            len
        }
    }
}

//

/// Multitasking API
pub struct Minimult<'a>
{
    alloc: MTAlloc<'a>
}

impl<'a> Minimult<'a>
{
    // Main context

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
    /// * (`num_tasks` * XXX) bytes of the memory block is consumed.
    pub fn new<B>(mem: &mut MTMemBlk<B>, num_tasks: MTTaskId) -> Minimult
    {
        let mut alloc = MTAlloc::new(mem);

        unsafe {
            O_TASKMGR = Some(MTTaskMgr::new(alloc.array(num_tasks), alloc.array(num_tasks)));
        }

        Minimult {
            alloc
        }
    }

    /// Creates a message queue.
    /// * `M` - type of the message element.
    /// * `len` - length of the message queue array.
    /// * Returns the created message queue.
    /// * (`len` * (size of `M` + XXX)) bytes of the memory block is consumed.
    pub fn msgq<M>(&mut self, len: usize) -> MTMsgQueue<'a, M> // TODO: lifetime is correct?
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
    where T: FnOnce() + Send + 'a  // TODO: lifetime is correct?
    {
        let tm = unsafe { O_TASKMGR.as_mut().unwrap() };

        let stack = self.alloc.array(stack_len);
        
        tm.register_once(tid, pri, stack, task);
    }

    /// Runs into a loop to dispatch the registered tasks.
    /// * Never returns.
    pub fn run(self) -> !
    {
        let control = cortex_m::register::control::read();
        assert!(control.spsel().is_msp()); // CONTROL.SPSEL: SP_main

        let scb_ptr = SCB::ptr();
        unsafe {
            (*scb_ptr).aircr.write(0x05fa0700); // PRIGROUP: 7 - no exception preempts each other
        }

        unsafe {
            LOOP_STARTED = true;
        }

        Minimult::dispatch();

        loop {
            cortex_m::asm::wfi(); // sleep to wait interrupt
        }
    }

    // Task and Interrupt context

    /// Makes a service call to request dispatching.
    pub fn dispatch()
    {
        unsafe {
            if !LOOP_STARTED {
                return;
            }
        }

        SCB::set_pendsv();
    }

    /// Brings a current running task into an idle state.
    pub fn idle()
    {
        unsafe {
            if let Some(tm) = O_TASKMGR.as_mut() {
                tm.idle();
            }
        }
    }

    /// Wakes up a task in an idle state.
    /// * `tid` - task identifier. `0` to `num_tasks - 1`.
    pub fn kick(tid: MTTaskId)
    {
        unsafe {
            if let Some(tm) = O_TASKMGR.as_mut() {
                tm.kick(tid);
            }
        }
    }

    fn wait()
    {
        unsafe {
            if let Some(tm) = O_TASKMGR.as_mut() {
                tm.wait();
            }
        }
    }

    fn signal(tid: MTTaskId)
    {
        unsafe {
            if let Some(tm) = O_TASKMGR.as_mut() {
                tm.signal(tid);
            }
        }
    }

    /// Gets task identifier of a current running task if any.
    /// Returns task identifier in `Option`.
    pub fn curr_tid() -> Option<MTTaskId>
    {
        unsafe {
            if let Some(tm) = O_TASKMGR.as_ref() {
                tm.tid
            }
            else {
                None
            }
        }
    }
}

impl Drop for Minimult<'_>
{
    fn drop(&mut self)
    {
        panic!(); // better message
    }
}

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
    fn new(mem: MTRawArray<Option<M>>) -> MTMsgQueue<'a, M> // TODO: lifetime is correct?
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
    pub fn send(&self, msg: M)
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

        q.wr_idx = next_wr_idx; // TODO: atomic access in case

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
    pub fn receive<F>(&self, f: F)
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

        q.rd_idx = next_rd_idx; // TODO: atomic access in case

        if let Some(wr_tid) = q.wr_tid {
            Minimult::signal(wr_tid);
        }
    }
} 
