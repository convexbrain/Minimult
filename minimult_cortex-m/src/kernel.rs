use cortex_m::peripheral::SCB;
use core::mem::{size_of, align_of, transmute};

use crate::{MTTaskId, MTTaskPri};
use crate::memory::MTRawArray;

//

#[no_mangle]
extern fn minimult_save_sp(curr_sp: *mut usize) -> *mut usize
{
    if let Some(tm) = mtkernel_get_mut() {
        tm.save_sp(curr_sp)
    }
    else {
        curr_sp
    }
}

#[no_mangle]
extern fn minimult_task_switch(sp: *mut usize) -> *mut usize
{
    if let Some(tm) = mtkernel_get_mut() {
        tm.task_switch()
    }
    else {
        sp
    }
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

static mut O_MTKERNEL: Option<MTKernel> = None;

pub(crate) fn mtkernel_create(tasks: MTRawArray<MTTask>, task_tree_array: MTRawArray<Option<(MTTaskId, MTTaskPri)>>)
{
    unsafe {
        O_MTKERNEL = Some(MTKernel::new(tasks, task_tree_array));
    }
}

pub(crate) fn mtkernel_get_ref() -> Option<&'static MTKernel>
{
    unsafe {
        O_MTKERNEL.as_ref()
    }
}

pub(crate) fn mtkernel_get_mut() -> Option<&'static mut MTKernel>
{
    unsafe {
        O_MTKERNEL.as_mut()
    }
}

//

fn align_down<A>(x: usize) -> usize
{
    let align = align_of::<A>();
    let y = x / align;
    let y = y * align;
    y
}

fn inf_loop() -> !
{
    let tm = mtkernel_get_mut().unwrap();
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

//

#[derive(Clone, Copy, PartialEq, Debug)]
enum MTState
{
    None,
    Idle,
    Ready,
    Waiting
}

pub(crate) struct MTTask
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

//

pub(crate) struct MTKernel
{
    tasks: MTRawArray<MTTask>,
    task_tree: MTBHeapDList<MTTaskId, MTTaskPri>,
    //
    is_set: bool,
    sp_loops: *mut usize,
    tid: Option<MTTaskId>
}

struct RefFnOnce
{
    data: *const u8,
    vtbl: *const usize
}

impl MTKernel
{
    // ----- ----- Main context ----- ----- //

    fn new(tasks: MTRawArray<MTTask>, task_tree_array: MTRawArray<Option<(MTTaskId, MTTaskPri)>>) -> MTKernel
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

        MTKernel {
            tasks,
            task_tree: MTBHeapDList::new(task_tree_array),
            is_set: false,
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

    pub(crate) fn register_once<T>(&mut self, tid: MTTaskId, pri: MTTaskPri, stack: MTRawArray<usize>, t: T)
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

        MTKernel::setup_task_once(sp, data, call_once);

        assert!(sp >= sp_start); // TODO: better message
        assert!(sp <= sp_end); // TODO: better message

        task.sp_start = sp_start;
        task.sp_end = sp_end;
        task.sp = sp;
        task.state = MTState::Ready;

        self.task_tree.add_bheap(tid, pri);
    }

    pub(crate) fn run(&mut self) -> !
    {
        let control = cortex_m::register::control::read();
        assert!(control.spsel().is_msp()); // CONTROL.SPSEL: SP_main

        let scb_ptr = SCB::ptr();
        unsafe {
            (*scb_ptr).aircr.write(0x05fa0700); // PRIGROUP: 7 - no exception preempts each other
        }

        self.is_set = true;

        self.dispatch();

        loop {
            cortex_m::asm::wfi(); // sleep to wait interrupt
        }
    }

    // ----- ----- Interrupt context ----- ----- //

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

    // ----- ----- Task context ----- ----- //

    fn none(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::None; // TODO: atomic access in case
        
        self.dispatch();
    }

    pub(crate) fn idle(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::Idle; // TODO: atomic access in case
        
        self.dispatch();
    }

    pub(crate) fn wait(&mut self)
    {
        let task = self.task_current().unwrap();

        task.state = MTState::Waiting; // TODO: atomic access in case
        
        self.dispatch();
    }

    pub(crate) fn signal(&mut self, tid: MTTaskId)
    {
        let task = self.tasks.refer(tid);

        ex_cntup(&mut task.signal_excnt);

        self.dispatch();
    }

    // ----- ----- Task and Interrupt context ----- ----- //

    fn task_current(&mut self) -> Option<&mut MTTask>
    {
        if let Some(curr_tid) = self.tid {
            Some(self.tasks.refer(curr_tid))
        }
        else {
            None
        }
    }

    pub(crate) fn dispatch(&self)
    {
        if self.is_set {
            SCB::set_pendsv();
        }
    }

    pub(crate) fn kick(&mut self, tid: MTTaskId)
    {
        let task = self.tasks.refer(tid);

        ex_cntup(&mut task.kick_excnt);

        self.dispatch();
    }

    pub(crate) fn curr_tid(&self) -> Option<MTTaskId>
    {
        self.tid
    }
}
