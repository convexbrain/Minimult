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

fn task0(snd: MTMsgSender<u32>)
{
    // other codes...

    loop {
        Minimult::idle();

        // other codes...

        let some_value = 1;
        snd.send(some_value);
    }
}

fn task1(rcv: MTMsgReceiver<u32>)
{
    // other codes...

    loop {
        let mut some_value = 0;
        rcv.receive(|v| {some_value = *v});

        // other codes...
    }
}
