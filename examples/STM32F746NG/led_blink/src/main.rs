#![no_main]
#![no_std]

use cortex_m::asm;
use cortex_m::peripheral::NVIC;
use cortex_m::Peripherals;
use cortex_m_rt::entry;
use cortex_m_rt::exception;
use panic_semihosting as _;

use stm32f7xx_hal::{
    device::{self, interrupt, Interrupt},
    prelude::*, gpio::*, timer::*};

use minimult_cortex_m::*;

//

const CLOCK: u32 = 216_000_000;
struct Toggle(u32, u32);

#[entry]
fn main() -> ! {
    let peri = device::Peripherals::take().unwrap();

    let mut rcc = peri.RCC.constrain();
    let clocks = rcc.cfgr.sysclk(216.mhz()).freeze();

    // ----- ----- ----- ----- -----

    let mut mem = Minimult::mem::<[u8; 4096]>();
    let mut mt = Minimult::new(&mut mem, 3);

    // ----- ----- ----- ----- -----

    let gpioi = peri.GPIOI.split();
    let pi1 = gpioi.pi1.into_push_pull_output();

    // ----- ----- ----- ----- -----

    let mut timer2 = Timer::tim2(peri.TIM2, 1.hz(), clocks, &mut rcc.apb1);
    timer2.listen(Event::TimeOut);
    let tim2cnt = 2; // NOTE: ??? need to adjust to make it 1hz

    unsafe { NVIC::unmask(Interrupt::TIM2) }

    // ----- ----- ----- ----- -----

    let cmperi = Peripherals::take().unwrap();
    let mut syst = cmperi.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(CLOCK / 16 - 1);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();
    let systcnt = 36; // 36/16=2.25 sec

    // ----- ----- ----- ----- -----

    let cnt0 = CLOCK / 8;
    let div0 = 1;
    let cnt1 = CLOCK;
    let div1 = 4;

    // message queue
    let mut q = mt.msgq::<Toggle>(4);
    let (snd, rcv) = q.ch();

    // shared message sender
    let s_snd = mt.share(snd);
    let sc_snd0 = s_snd.ch();
    let sc_snd1 = s_snd.ch();

    mt.register(0, 1, 256, || _led_tim0(tim2cnt, sc_snd0, cnt0, div0));
    mt.register(1, 1, 256, || _led_tim1(systcnt, sc_snd1, cnt1, div1));
    mt.register(2, 2, 256, || _led_tgl(pi1, rcv)); // blink and pause

    // ----- ----- ----- ----- -----

    mt.run() // NOTE: inside WFI may block SysTick in some cases
}

fn _led_tgl(mut pi1: gpioi::PI1<Output<PushPull>>, mut rcv: MTMsgReceiver<Toggle>)
{
    let mut tgl = Toggle(CLOCK / 16, 1);

    loop {
        for _ in 0..tgl.1 {
            pi1.set_high().unwrap();

            asm::delay(tgl.0 / 4 / tgl.1);

            pi1.set_low().unwrap();

            asm::delay(tgl.0 / 4 / tgl.1);
        }

        pi1.set_low().unwrap();

        asm::delay(tgl.0 / 2);

        tgl = rcv.receive();
    }
}

fn _led_tim0(timcnt: u32, sc_snd: MTSharedCh<MTMsgSender<Toggle>>, cnt: u32, div: u32)
{
    loop {
        for _ in 0..timcnt {
            Minimult::idle();

            //

            // NOTE: ??? unsafe way to clear UIF
            let tim2_sr = 0x4000_0010 as *mut u32;
            unsafe {
                let r = tim2_sr.read_volatile();
                tim2_sr.write_volatile(r & 0xfffffffe);
            }
            NVIC::unpend(Interrupt::TIM2);
            unsafe { NVIC::unmask(Interrupt::TIM2) }
        }

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
fn TIM2()
{
    NVIC::mask(Interrupt::TIM2);
    
    Minimult::kick(0);
}

#[exception]
fn SysTick()
{
    Minimult::kick(1);
}
