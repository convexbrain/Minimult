use core::mem::{size_of, align_of, transmute};

use crate::{MTTaskId, MTTaskPri};
use crate::memory::MTRawArray;
use crate::bheap::MTBHeapDList;
use crate::bk_panic;
use crate::bkptpanic::BKUnwrap;

//

/*
Refer exception entry behavior of ARM v6/7/8-M Architecture Reference Manual

sp+
0-9: [context preservation by SW at PendSV]
    R8      R9      R10     R11     R4      R5      R6      R7
    (Rsvd.) LR(exc) 
10-17: [Basic frame saved by HW at exception entry]
    R0      R1      R2      R3      R12     LR(R14) RetAddr xPSR
18-35: [Extended frame saved by HW at exception entry]
    S0      S1      S2      S3      S4      S5      S6      S7
    S8      S9      S10     S11     S12     S13     S14     S15
    FPSCR   (Rsvd.)
                    ^ 8-byte aligned here
*/

#[no_mangle]
extern "C" fn minimult_save_sp(curr_sp: *mut usize, curr_splim: *mut usize) -> (*mut usize, *mut usize)
{
    if let Some(tm) = mtkernel_get_mut() {
        tm.save_sp(curr_sp, curr_splim)
    }
    else {
        (curr_sp, curr_splim)
    }
}

#[no_mangle]
extern "C" fn minimult_task_switch(sp: *mut usize, splim: *mut usize) -> (*mut usize, *mut usize)
{
    if let Some(tm) = mtkernel_get_mut() {
        tm.task_switch()
    }
    else {
        (sp, splim)
    }
}

fn setup_stack(sp: *mut usize, data: *mut u8, call_once: usize, inf_loop: fn() -> !) -> *mut usize
{
    let sp = sp as usize;
    let sp = align_down::<u64>(sp); // 8-byte align
    let sp = sp as *mut usize;

    unsafe {
        let sp = sp.sub(18 + 2/*margin*/);

        // LR(exc): Return to Thread mode, Returb stack Main, Frame type Basic
        sp.add(9).write_volatile(0xffff_fff9);
        
        // R0
        sp.add(10 + 0).write_volatile(data as usize);
        
        // LR(R14)
        sp.add(10 + 5).write_volatile(inf_loop as usize);

        // RetAddr
        sp.add(10 + 6).write_volatile(call_once);

        // xPSR: set T-bit since Cortex-M has only Thumb instructions
        sp.add(10 + 7).write_volatile(0x01000000);

        sp
    }
}

//

extern "C" {
    fn minimult_ex_incr(exc: &mut usize);
    fn minimult_ex_decr(exc: &mut usize);
    fn minimult_ex_incr_ifgt0(exc: &mut usize) -> usize;
    fn minimult_ex_decr_if1(exc: &mut usize) -> usize;
}

//

static mut O_MTKERNEL: Option<MTKernel> = None;

pub(crate) fn mtkernel_create(tasks: MTRawArray<MTTask>, task_tree_array: MTRawArray<(MTTaskId, MTTaskPri)>)
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
    let tm = mtkernel_get_mut().bk_unwrap();
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
    Equal(usize),
    NotEqual(usize),
    LessThan(usize),
    GreaterThan(usize)
}

pub(crate) struct MTEvent
{
    ex_cnt: usize
}

impl MTEvent
{
    pub(crate) fn new(init_cnt: usize) -> MTEvent
    {
        MTEvent {
            ex_cnt: init_cnt
        }
    }

    pub(crate) fn cnt(&self) -> usize
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

    pub(crate) fn incr_ifgt0(&mut self) -> bool
    {
        unsafe {
            minimult_ex_incr_ifgt0(&mut self.ex_cnt) > 0 // NOTE: wrapping-around not checked
        }
    }

    pub(crate) fn decr_if1(&mut self) -> bool
    {
        unsafe {
            minimult_ex_decr_if1(&mut self.ex_cnt) > 0 // NOTE: wrapping-around not checked
        }
    }

    fn cond_matched(&self, cond: &MTEventCond) -> bool
    {
        match cond {
            MTEventCond::None => {
                bk_panic!()
            }
            MTEventCond::Equal(target) => {
                self.cnt() == *target
            }
            MTEventCond::NotEqual(target) => {
                self.cnt() != *target
            }
            MTEventCond::LessThan(target) => {
                self.cnt() < *target
            }
            MTEventCond::GreaterThan(target) => {
                self.cnt() > *target
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
    wait_evcond: MTEventCond,
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
    splim_loops: *mut usize,
    tid: Option<MTTaskId>
}

impl MTKernel
{
    // ----- ----- Main context ----- ----- //

    fn new(tasks: MTRawArray<MTTask>, task_tree_array: MTRawArray<(MTTaskId, MTTaskPri)>) -> MTKernel
    {
        for i in 0..tasks.len() {
            tasks.write(i,
                MTTask {
                    sp_start: core::ptr::null_mut(),
                    sp_end: core::ptr::null_mut(),
                    sp: core::ptr::null_mut(),
                    state: MTState::None,
                    wait_ev: core::ptr::null_mut(),
                    wait_evcond: MTEventCond::None,
                    idle_kick_ev: MTEvent::new(0)
                }
            );
        }

        MTKernel {
            tasks,
            task_tree: MTBHeapDList::new(task_tree_array),
            is_set: false,
            sp_loops: core::ptr::null_mut(),
            splim_loops: core::ptr::null_mut(),
            tid: None
        }
    }

    pub(crate) fn register_once<T>(&mut self, tid: MTTaskId, pri: MTTaskPri, stack: MTRawArray<usize>, t: T)
    where T: FnOnce() + Send // NOTE: unsafe lifetime
    {
        assert!((tid as usize) < self.tasks.len(),
                "tid {}: out of number of tasks", tid);

        let task = self.tasks.refer(tid);

        assert!(task.state == MTState::None,
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
        assert!(control.spsel().is_msp(),
                "CONTROL.SPSEL: must be SP_main");

        let scb_ptr = cortex_m::peripheral::SCB::ptr();
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

    fn save_sp(&mut self, curr_sp: *mut usize, curr_splim: *mut usize) -> (*mut usize, *mut usize)
    {
        // check and save current sp

        if let Some(task) = self.task_current() {
            assert!((curr_sp >= task.sp_start) && (curr_sp <= task.sp_end),
                    "tid {}: stack shortage", self.tid.bk_unwrap());

            task.sp = curr_sp;
        }
        else {
            self.sp_loops = curr_sp;
            self.splim_loops = curr_splim;
        }

        (self.sp_loops, self.splim_loops) // use sp[lim]_loops until switching task
    }

    fn task_switch(&mut self) -> (*mut usize, *mut usize)
    {
        // clear service call request

        cortex_m::peripheral::SCB::clear_pendsv();

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
                    let ev = unsafe { task.wait_ev.as_ref().bk_unwrap() };
                    
                    if ev.cond_matched(&task.wait_evcond) {
                        task.state = MTState::Ready;
                        true
                    }
                    else {
                        false
                    }
                }
                _ => bk_panic!()
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

        let (next_tid, next_sp, next_splim) = if let Some(tid) = self.task_tree.bheap_h() {
            (Some(tid), self.tasks.refer(tid).sp, self.tasks.refer(tid).sp_start)
        }
        else {
            (None, self.sp_loops, self.splim_loops)
        };

        self.tid = next_tid;

        (next_sp, next_splim)
    }

    // ----- ----- Task context ----- ----- //

    fn none(&mut self)
    {
        let task = self.task_current().bk_unwrap();

        task.state = MTState::None; // NOTE: atomic access might be necessary
        
        self.dispatch();
    }

    pub(crate) fn idle(&mut self)
    {
        loop {
            let task = self.task_current().bk_unwrap();

            if task.idle_kick_ev.cnt() != 0 {
                task.idle_kick_ev.decr();
                break;
            }

            task.wait_ev = &task.idle_kick_ev;
            task.wait_evcond = MTEventCond::NotEqual(0);
            task.state = MTState::Waiting; // NOTE: atomic access might be necessary
            
            self.dispatch();
        }
    }

    pub(crate) fn wait(&mut self, ev: &MTEvent, evcond: MTEventCond)
    {
        let task = self.task_current().bk_unwrap();

        task.wait_ev = ev;
        task.wait_evcond = evcond;
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
            cortex_m::peripheral::SCB::set_pendsv();
        }
    }

    pub(crate) fn kick(&mut self, tid: MTTaskId)
    {
        assert!((tid as usize) < self.tasks.len(),
                "tid {}: out of number of tasks", tid);
        
        let task = self.tasks.refer(tid);

        task.idle_kick_ev.incr();
        
        self.dispatch(); // NOTE: room of optimization using ev
    }

    pub(crate) fn curr_tid(&self) -> Option<MTTaskId>
    {
        self.tid
    }
}
