/*!
This crate for Rust provides a minimal multitask library `Minimult` for Cortex-M microcontrollers.

# Target

*Single-core* systems of

* Cortex-M0 / M0+ / M1  (`thumbv6m-none-eabi`)
* Cortex-M3  (`thumbv7m-none-eabi`)
* Cortex-M4 / M7  (`thumbv7em-none-eabi`) with FPU  (`thumbv7em-none-eabihf`)
* Cortex-M23  (`thumbv8m.base-none-eabi`)
* Cortex-M33 / M35P  (`thumbv8m.main-none-eabi`) with FPU  (`thumbv8m.main-none-eabihf`)

# Features

* Task like that of a typical RTOS
  * `Minimult` can take closures and register them as tasks.
  * `Minimult` runs into a loop to start dispatching those tasks.
    * *Not supported: dynamically creating and spawning.*
* Synchronization
  * `idle` and `kick`
    * A task goes into an idle state and other tasks/interrupts wake it up by kicking.
  * `MTMsgSender` and `MTMsgReceiver`
    * Task-to-task communication by message passing.
  * `MTSharedCh`
    * Shared variable among tasks.
* Priority-based dispatching
  * A higher priority task preempts lower priority tasks.
  * Round-robin dispatching within the same priority tasks.
  * `dispatch` can be directly requested so that timer-based preemption is also possible.
* Static memory allocation
  * `Minimult` doesn't require a global allocator but reserves a bunch of memory block in advance.

# Examples
## Usage Outline

```no_run
// Build-only example

#![no_main]
#![no_std]

use cortex_m::Peripherals;
use cortex_m_rt::entry;
use cortex_m_rt::exception;
extern crate panic_semihosting;

// other codes...

use minimult_cortex_m::*;

#[entry]
fn main() -> ! {
    let mut mem = Minimult::mem::<[u8; 4096]>();
    let mut mt = Minimult::new(&mut mem, 2);

    // other codes...

    let mut q = mt.msgq::<u32>(4);
    let (snd, rcv) = q.ch();

    mt.register(0/*tid*/, 1, 256, || task0(snd));
    mt.register(1/*tid*/, 1, 256, || task1(rcv));

    // other codes...

    let cmperi = Peripherals::take().unwrap();
    let mut syst = cmperi.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(0xffffff);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();

    // other codes...

    mt.run()
}

#[exception]
fn SysTick()
{
    // other codes...

    Minimult::kick(0/*tid*/);
}

fn task0(mut snd: MTMsgSender<u32>)
{
    // other codes...

    loop {
        Minimult::idle();

        // other codes...

        let some_value = 1;
        snd.send(some_value);
    }
}

fn task1(mut rcv: MTMsgReceiver<u32>)
{
    // other codes...

    loop {
        let some_value = rcv.receive();
        assert_eq!(some_value, 1);

        // other codes...
    }
}
```

## Other Examples

You can find a specific board's example [here](https://github.com/convexbrain/Minimult/tree/master/examples/).
Currently there are very few examples, however.
*/

#![no_std]

/*
Type parameter rule:
 * A: method-level general variable
 * F: method-level general closure
 * V: struct-level general variable
 * T: task closure
 * I: index
 * K: key
 * M: message
 * B: memory block
*/

mod minimult;  // Lifetime safe and high-level API wrapper
mod kernel;    // Low-level unsafe and lifetime unbounded singleton
mod bheap;     // binary heap and list
mod memory;    // static memory allocation
mod msgqueue;  // message queue
mod shared;    // read-write shared variable
mod bkptpanic; // bkpt panic, assert and unwrap

/// Task identifier
pub type MTTaskId = u16;

/// Task priority
pub type MTTaskPri = u8;

pub use crate::minimult::{
    Minimult
};

pub use crate::memory::{
    MTMemBlk
};

pub use crate::msgqueue::{
    MTMsgSender, MTMsgReceiver,
    MTMsgQueue
};

pub use crate::shared::{
    MTSharedCh,
    MTShared, MTSharedLook, MTSharedTouch
};
