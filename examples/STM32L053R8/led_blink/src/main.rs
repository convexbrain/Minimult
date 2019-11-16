#![no_main]
#![no_std]

use cortex_m::asm;
use cortex_m::peripheral::NVIC;
use cortex_m::Peripherals;
use cortex_m_rt::entry;
use cortex_m_rt::exception;
use panic_semihosting as _;

use stm32l0xx_hal::{
    pac::{self, interrupt, Interrupt},
    prelude::*, rcc::*, gpio::*, timer::*};

use minimult_cortex_m::*;

//

const CLOCK: u32 = 16_000_000;
struct Toggle(u32, u32);

#[entry]
fn main() -> ! {
    let peri = pac::Peripherals::take().unwrap();
    let mut rcc = peri.RCC.freeze(Config::hsi16());

    // ----- ----- ----- ----- -----

    let mut mem = Minimult::mem::<[u8; 4096]>();
    let mut mt = Minimult::new(&mut mem, 3);

    // ----- ----- ----- ----- -----

    let gpioa = peri.GPIOA.split(&mut rcc);
    let pa5 = gpioa.pa5.into_push_pull_output();

    // ----- ----- ----- ----- -----

    let mut timer2 = peri.TIM2.timer(1.hz(), &mut rcc);
    timer2.listen();

    unsafe { NVIC::unmask(Interrupt::TIM2) }

    // ----- ----- ----- ----- -----

    let cmperi = Peripherals::take().unwrap();
    let mut syst = cmperi.SYST;
    syst.set_clock_source(cortex_m::peripheral::syst::SystClkSource::Core);
    syst.set_reload(CLOCK / 4 - 1);
    syst.clear_current();
    syst.enable_counter();
    syst.enable_interrupt();
    let systcnt = 9; // 9/4=2.25 sec

    // ----- ----- ----- ----- -----

    let cnt0 = CLOCK / 32;
    let div0 = 1;
    let cnt1 = CLOCK / 4;
    let div1 = 4;

    // message queue
    let mut q = mt.msgq::<Toggle>(4);
    let (snd, rcv) = q.ch();

    // shared message sender
    let s_snd = mt.share(snd);
    let sc_snd0 = s_snd.ch();
    let sc_snd1 = s_snd.ch();

    mt.register(0, 1, 256, || _led_tim0(timer2, sc_snd0, cnt0, div0));
    mt.register(1, 1, 256, || _led_tim1(systcnt, sc_snd1, cnt1, div1));
    mt.register(2, 2, 256, || _led_tgl(pa5, rcv)); // blink and pause

    // ----- ----- ----- ----- -----

    mt.run() // NOTE: inside WFI may block SysTick in some cases
}

fn _led_tgl(mut pa5: gpioa::PA5<Output<PushPull>>, mut rcv: MTMsgReceiver<Toggle>)
{
    let mut tgl = Toggle(CLOCK / 16, 1);

    loop {
        for _ in 0..tgl.1 {
            pa5.set_high().unwrap();

            asm::delay(tgl.0 / 4 / tgl.1);

            pa5.set_low().unwrap();

            asm::delay(tgl.0 / 4 / tgl.1);
        }

        pa5.set_low().unwrap();

        asm::delay(tgl.0 / 2);

        tgl = rcv.receive();
    }
}

fn _led_tim0(mut timer2: Timer<pac::TIM2>, sc_snd: MTSharedCh<MTMsgSender<Toggle>>, cnt: u32, div: u32)
{
    loop {
        Minimult::idle();

        //

        timer2.clear_irq();
        NVIC::unpend(Interrupt::TIM2);
        unsafe { NVIC::unmask(Interrupt::TIM2) }

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
