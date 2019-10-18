// UNDER DEVELOPMENT AND EXPERIMENT
// TODO: check if stable toolchain is enough
// TODO: check release build

#![no_main]
#![no_std]

use cortex_m::asm;
use cortex_m::peripheral::NVIC;

use cortex_m_rt::entry;


extern crate panic_semihosting;
/*
use core::panic::PanicInfo;
#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        asm::bkpt();
    }
}
*/


use nrf52840_pac::{
    P0, TIMER0,
    interrupt, Interrupt};


use minimult_cortex_m::*;


struct Count(u32);

#[entry]
fn main() -> ! {
    let mut mem = Minimult::memory::<[u8; 4096]>();
    let mut mt = Minimult::create(&mut mem, 2);

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

    let cycles = 1_000_000 * 2;
    timer0.cc[0].write(|w| unsafe { w.cc().bits(cycles) }); // 2 sec
    timer0.tasks_clear.write(|w| w.tasks_clear().set_bit());
    timer0.tasks_start.write(|w| w.tasks_start().set_bit());

    // ----- ----- ----- ----- -----

    let mut que = mt.msgq::<u32>(4);
    let (snd, rcv) = que.ch();

    let v1 = Count(4);
    let v2 = Count(16);

    mt.register(0, 1, 256, || led_cnt(timer0, snd, &v1, &v2));
    mt.register(1, 1, 256, || _led_tgl1(p0, rcv)); // blink and pause
    //mt.register(1, 1, 256, || _led_tgl2(p0, rcv)); // keep blinking

    //core::mem::drop(que); // must be error
    //core::mem::drop(v1); // must be error
    //core::mem::drop(mem); // must be error
    
    // ----- ----- ----- ----- -----

    mt.loops()
}

fn _led_tgl1(p0: P0, rcv: MTMsgReceiver<u32>)
{
    let cnt_half = 64_000_000 / 4;
    let mut div = 2;

    loop {
        for _ in 0..div {
            p0.outset.write(|w| w.pin7().set_bit());

            asm::delay(cnt_half / div);

            p0.outclr.write(|w| w.pin7().set_bit());

            asm::delay(cnt_half / div);
        }

        rcv.receive(|v| {div = *v});
    }
}

fn _led_tgl2(p0: P0, rcv: MTMsgReceiver<u32>)
{
    let cnt_half = 64_000_000 / 4;
    let mut div = 1;

    loop {
        while rcv.available() > 0 {
            rcv.receive(|v| {div = *v});
        }

        p0.outset.write(|w| w.pin7().set_bit());

        asm::delay(cnt_half / div);

        p0.outclr.write(|w| w.pin7().set_bit());

        asm::delay(cnt_half / div);
    }
}

fn led_cnt(timer0: TIMER0, snd: MTMsgSender<u32>, cnt_1: &Count, cnt_2: &Count)
{
    let mut flag = true;

    loop {
        Minimult::idle();

        //

        timer0.events_compare[0].write(|w| {w.events_compare().bit(false)});
        NVIC::unpend(Interrupt::TIMER0);
        unsafe { NVIC::unmask(Interrupt::TIMER0) }

        //

        snd.send(if flag {cnt_1.0} else {cnt_2.0});
        flag = !flag;
    }
}

#[interrupt]
fn TIMER0()
{
    NVIC::mask(Interrupt::TIMER0);
    
    Minimult::kick(0);
}
