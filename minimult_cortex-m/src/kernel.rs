use cortex_m::peripheral::SCB;
use core::mem::{size_of, align_of, transmute};

use crate::{MTTaskId, MTTaskPri};
use crate::memory::MTRawArray;
use crate::bheap::MTBHeapDList;

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

//

extern "C" {
    fn minimult_ex_incr(exc: &mut isize);
    fn minimult_ex_decr(exc: &mut isize);
}

fn setup_stack(sp: *mut usize, data: *mut u8, call_once: usize, inf_loop: fn() -> !) -> *mut usize
{
    /*
     * Magic numbers from exception entry behavior of ARM v6/7/8-M Architecture Reference Manual
     */

    let sp = sp as usize;
    let sp = align_down::<u64>(sp); // 8-byte align
    let sp = sp as *mut usize;

    unsafe {
        let framesize = if cfg!(has_fpu) {
            0x68 / 4
        }
        else {
            0x20 / 4
        };

        let sp = sp.sub(framesize + 2/*margin*/);

        // r0
        sp.add(8 + 0).write_volatile(data as usize);
        
        // lr
        sp.add(8 + 5).write_volatile(inf_loop as usize);

        // ReturnAddress
        sp.add(8 + 6).write_volatile(call_once);

        // xPSR: set T-bit since Cortex-M has only Thumb instructions
        sp.add(8 + 7).write_volatile(0x01000000);

        sp
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

#[derive(PartialEq, Debug)]
enum MTState
{
    None,
    Ready,
    Waiting
}

pub(crate) enum MTEventCond
{
    None,
    NotEqual(isize),
    LessThan(isize),
    MoreThan(isize)
}

pub(crate) struct MTEvent
{
    ex_cnt: isize,
    cond: MTEventCond
}

impl MTEvent
{
    pub(crate) fn new() -> MTEvent
    {
        MTEvent {
            ex_cnt: 0,
            cond: MTEventCond::None
        }
    }

    pub(crate) fn cnt(&self) -> isize
    {
        self.ex_cnt
    }

    pub(crate) fn incr(&mut self)
    {
        unsafe {
            minimult_ex_incr(&mut self.ex_cnt); // NOTE: wrapping-around not checked
        }
    }

    pub(crate) fn decr(&mut self)
    {
        unsafe {
            minimult_ex_decr(&mut self.ex_cnt); // NOTE: wrapping-around not checked
        }
    }

    pub(crate) fn set_cond(&mut self, cond: MTEventCond)
    {
        self.cond = cond;
    }

    fn cond_matched(&self) -> bool
    {
        match self.cond {
            MTEventCond::None => false,
            MTEventCond::NotEqual(target) => {
                self.ex_cnt != target
            }
            MTEventCond::LessThan(target) => {
                self.ex_cnt < target
            }
            MTEventCond::MoreThan(target) => {
                self.ex_cnt > target
            }
        }
    }
}

//

pub(crate) struct MTTask
{
    sp_start: *mut usize,
    sp_end: *mut usize,
    //
    sp: *mut usize,
    state: MTState,
    wait_ev: *const MTEvent,
    //
    idle_kick_ev: MTEvent
}

struct RefFnOnce
{
    data: *const u8,
    vtbl: *const usize
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
                    state: MTState::None,
                    wait_ev: core::ptr::null_mut(),
                    idle_kick_ev: MTEvent::new()
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

    pub(crate) fn register_once<T>(&mut self, tid: MTTaskId, pri: MTTaskPri, stack: MTRawArray<usize>, t: T)
    where T: FnOnce() + Send // NOTE: unsafe lifetime
    {
        let task = self.tasks.refer(tid);

        assert_eq!(task.state, MTState::None,
                   "tid {}: double registration", tid);

        let sp_start = stack.head();
        let sp_end = stack.tail();

        let sz = size_of::<T>();
        let rfo = unsafe { transmute::<&dyn FnOnce(), RefFnOnce>(&t) };

        let sp = sp_end as usize;
        let sp = align_down::<T>(sp - sz);
        let sp = sp as *mut usize;

        let data = sp as *mut u8;
        unsafe {
            core::intrinsics::copy(rfo.data, data, sz)
        }

        let vtbl = rfo.vtbl;
        /* 
         * NOTE: rustc 1.38.0 (625451e37 2019-09-23) places call_once at vtbl[3].
         */
        let call_once = unsafe { vtbl.add(3).read() };

        let sp = setup_stack(sp, data, call_once, inf_loop);

        assert!((sp >= sp_start) && (sp <= sp_end),
                "tid {}: stack shortage");

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
            assert!((curr_sp >= task.sp_start) && (curr_sp <= task.sp_end),
                    "tid {}: stack shortage", self.tid.unwrap());

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
                MTState::Waiting => {
                    let ev = unsafe { task.wait_ev.as_ref().unwrap() };
                    
                    if ev.cond_matched() {
                        task.state = MTState::Ready;
                        true
                    }
                    else {
                        false
                    }
                }
                _ => panic!()
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

        task.state = MTState::None; // NOTE: atomic access might be necessary
        
        self.dispatch();
    }

    pub(crate) fn idle(&mut self)
    {
        loop {
            let task = self.task_current().unwrap();

            if task.idle_kick_ev.cnt() != 0 {
                task.idle_kick_ev.decr();
                break;
            }

            task.idle_kick_ev.set_cond(MTEventCond::NotEqual(0));
            
            task.wait_ev = &task.idle_kick_ev;
            task.state = MTState::Waiting; // NOTE: atomic access might be necessary
            
            self.dispatch();
        }
    }

    pub(crate) fn wait(&mut self, ev: &MTEvent)
    {
        let task = self.task_current().unwrap();

        task.wait_ev = ev;
        task.state = MTState::Waiting; // NOTE: atomic access might be necessary
        
        self.dispatch();
    }

    pub(crate) fn signal(&mut self, _ev: &MTEvent)
    {
        self.dispatch(); // NOTE: room of optimization using ev
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

        task.idle_kick_ev.incr();
        
        self.dispatch(); // NOTE: room of optimization using ev
    }

    pub(crate) fn curr_tid(&self) -> Option<MTTaskId>
    {
        self.tid
    }
}
