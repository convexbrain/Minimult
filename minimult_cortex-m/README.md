# minimult_cortex-m

This crate for Rust provides a minimal multitask library `Minimult` for Cortex-M microcontrollers.

## Target

*Single-core* systems of

* Cortex-M0 / M0+ / M1  (`thumbv6m-none-eabi`)
* Cortex-M3  (`thumbv7m-none-eabi`)
* Cortex-M4 / M7  (`thumbv7em-none-eabi`) with FPU  (`thumbv7em-none-eabihf`)
* Cortex-M23  (`thumbv8m.base-none-eabi`)
* Cortex-M33 / M35P  (`thumbv8m.main-none-eabi`) with FPU  (`thumbv8m.main-none-eabihf`)

## Features

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

## Examples
### Usage

```rust
// Runnable on QEMU ARM

#![no_main]
#![no_std]

use cortex_m::Peripherals;
use cortex_m_rt::entry;
use cortex_m_rt::exception;
use cortex_m_semihosting::debug;
use cortex_m_semihosting::hprintln;
use panic_semihosting as _;

use minimult_cortex_m::*;

#[entry]
fn main() -> !
{
    let mut mem = Minimult::mem::<[u8; 4096]>();
    let mut mt = Minimult::new(&mut mem, 3);

    let mut q = mt.msgq::<u32>(4);
    let (snd, rcv) = q.ch();

    let sh = mt.share::<u32>(0);
    let shch1 = sh.ch();
    let shch2 = sh.ch();

    mt.register(0/*tid*/, 1, 256, || task0(snd));
    mt.register(1/*tid*/, 1, 256, || task1(rcv, shch1));
    mt.register(2/*tid*/, 1, 256, || task2(shch2));

    // SysTick settings
    let cmperi = Peripherals::take().unwrap();
    let mut syst = cmperi.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(1_000_000);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();

    // must be error in terms of lifetime and ownership
    //drop(mem);
    //drop(q);
    //drop(snd);
    //drop(rcv);
    //drop(sh);
    //drop(shch1);
    //drop(shch2);

    hprintln!("Minimult run").unwrap();
    mt.run()
}

#[exception]
fn SysTick()
{
    Minimult::kick(0/*tid*/);
}

fn task0(mut snd: MTMsgSender<u32>)
{
    for vsnd in 0.. {
        Minimult::idle();

        hprintln!("task0 send {}", vsnd).unwrap();
        snd.send(vsnd);
    }
}

fn task1(mut rcv: MTMsgReceiver<u32>, shch: MTSharedCh<u32>)
{
    for i in 0.. {
        let vrcv = rcv.receive();

        assert_eq!(i, vrcv);
        hprintln!("task1 touch {}", vrcv).unwrap();
        let mut vtouch = shch.touch();
        *vtouch = vrcv;
    }
}

fn task2(shch: MTSharedCh<u32>)
{
    let mut j = 0;

    while j < 5 {
        let vlook = shch.look();

        assert!((j == *vlook) || (j + 1 == *vlook));
        //hprintln!("task2 look {}", *vlook).unwrap(); // many lines printed
        j = *vlook;
    }

    hprintln!("task2 exit").unwrap();
    debug::exit(debug::EXIT_SUCCESS);
}
```

### Other Examples

You can find a specific board's example [here](https://github.com/convexbrain/Minimult/tree/master/examples/).
Currently there are very few examples, however.
