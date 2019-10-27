#![no_main]
#![no_std]

use cortex_m::asm;
use cortex_m::peripheral::NVIC;
use cortex_m::Peripherals;

use cortex_m_rt::entry;
use cortex_m_rt::exception;

extern crate panic_semihosting;

use nrf52840_pac::{
    P0, TIMER0,
    interrupt, Interrupt};

use minimult_cortex_m::*;

//

struct Toggle(u32, u32);

#[entry]
fn main() -> ! {
    let mut mem = Minimult::mem::<[u8; 4096]>();
    let mut mt = Minimult::new(&mut mem, 3);

    // ----- ----- ----- ----- -----

    let peri = nrf52840_pac::Peripherals::take().unwrap();

    // ----- ----- ----- ----- -----

    let p0 = peri.P0;
    p0.outclr.write(|w| w.pin7().set_bit());
    p0.pin_cnf[7].write(|w| w
        .dir().output()
        .input().disconnect()
        .pull().disabled()
        .drive().s0s1()
        .sense().disabled());

    // ----- ----- ----- ----- -----

    let timer0 = peri.TIMER0;
    timer0.shorts.write(|w| w
        .compare0_clear().enabled()
        .compare0_stop().disabled());
    timer0.prescaler.write(|w| unsafe { w.prescaler().bits(4) }); // 1 MHz
    timer0.bitmode.write(|w| w.bitmode()._32bit());
    timer0.intenset.modify(|_, w| w.compare0().set());

    unsafe { NVIC::unmask(Interrupt::TIMER0) }

    let cycles = 1_000_000;
    timer0.cc[0].write(|w| unsafe { w.cc().bits(cycles) }); // 1 sec
    timer0.tasks_clear.write(|w| w.tasks_clear().set_bit());
    timer0.tasks_start.write(|w| w.tasks_start().set_bit());

    // ----- ----- ----- ----- -----

    let cmperi = Peripherals::take().unwrap();
    let mut syst = cmperi.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(16_000_000 - 1);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();
    let systcnt = 64_000_000/16_000_000 * 7/3; // 7/3 sec

    // ----- ----- ----- ----- -----

    let cnt0 = 64_000_000 / 32;
    let div0 = 1;
    let cnt1 = 64_000_000 / 4;
    let div1 = 4;

    /* using message queue
     */
    let mut q = mt.msgq::<Toggle>(4);
    let (snd, rcv) = q.ch();

    let s_snd = mt.share(snd);
    let sc_snd0 = s_snd.ch();
    let sc_snd1 = s_snd.ch();

    mt.register(0, 1, 256, || _led_tim0(timer0, sc_snd0, cnt0, div0));
    mt.register(1, 1, 256, || _led_tim1(systcnt, sc_snd1, cnt1, div1));
    mt.register(2, 2, 256, || _led_tgl(p0, rcv)); // blink and pause

    {   // must be error in terms of lifetime
        //core::mem::drop(mem);
        //core::mem::drop(q);
        //core::mem::drop(s_snd);
        //core::mem::drop(sc_snd0);
        //core::mem::drop(sc_snd1);
        //core::mem::drop(rcv);
    }
    
    // ----- ----- ----- ----- -----

    mt.run()
}

fn _led_tgl(p0: P0, mut rcv: MTMsgReceiver<Toggle>)
{
    let mut tgl = Toggle(64_000_000 / 16, 1);

    loop {
        for _ in 0..tgl.1 {
            p0.outset.write(|w| w.pin7().set_bit());

            asm::delay(tgl.0 / 4 / tgl.1);

            p0.outclr.write(|w| w.pin7().set_bit());

            asm::delay(tgl.0 / 4 / tgl.1);
        }

            p0.outclr.write(|w| w.pin7().set_bit());

            asm::delay(tgl.0 / 2);

        tgl = rcv.receive();
    }
}

fn _led_tim0(timer0: TIMER0, sc_snd: MTSharedCh<MTMsgSender<Toggle>>, cnt: u32, div: u32)
{
    loop {
        Minimult::idle();

        //

        timer0.events_compare[0].write(|w| {w.events_compare().bit(false)});
        NVIC::unpend(Interrupt::TIMER0);
        unsafe { NVIC::unmask(Interrupt::TIMER0) }

        //

        let mut snd = sc_snd.touch();
        snd.send(Toggle(cnt, div));
    }
}

fn _led_tim1(timcnt: u32, sc_snd: MTSharedCh<MTMsgSender<Toggle>>, cnt: u32, div: u32)
{
    loop {
        for _ in 0..timcnt {
            Minimult::idle();
        }

        //

        let mut snd = sc_snd.touch();
        snd.send(Toggle(cnt, div));
    }
}

#[interrupt]
fn TIMER0()
{
    NVIC::mask(Interrupt::TIMER0);
    
    Minimult::kick(0);
}

#[exception]
fn SysTick()
{
    Minimult::kick(1);
}
