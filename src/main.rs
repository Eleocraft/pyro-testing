#![no_std]
#![no_main]
#![feature(generic_const_exprs)]

mod adc;

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    adc::{Adc, AdcChannel}, exti::ExtiInput, gpio::{Level, Output, Pull, Speed}
};
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, watch::{Sender, Watch}};
use embassy_time::{Duration, Timer};
use crate::adc::{AdcCtrl, AdcCtrlChannel};

use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

fn reset(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_high();
    fire_a.set_low();
    safe_b.set_high();
    fire_b.set_low();
}

fn test_1(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_low();
    fire_a.set_low();
    safe_b.set_low();
    fire_b.set_low();
}

fn test_2(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_low();
    fire_a.set_high();
    safe_b.set_low();
    fire_b.set_low();
}

fn test_3(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_high();
    fire_a.set_low();
    safe_b.set_high();
    fire_b.set_low();
}

fn test_4(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_high();
    fire_a.set_high();
    safe_b.set_high();
    fire_b.set_low();
}

fn test_5(safe_a: &mut Output<'static>, fire_a: &mut Output<'static>, safe_b: &mut Output<'static>, fire_b: &mut Output<'static>) {
    safe_a.set_high();
    fire_a.set_high();
    safe_b.set_high();
    fire_b.set_high();
}

#[embassy_executor::task]
async fn run_tasks(
    sender: Sender<'static, ThreadModeRawMutex, usize, 1>,
    mut button: ExtiInput<'static>,
    mut safe_a: Output<'static>,
    mut fire_a: Output<'static>,
    mut safe_b: Output<'static>,
    mut fire_b: Output<'static>) {

    let tests = [test_1, test_2, test_3, test_4, test_5, reset];

    for (i, test) in tests.iter().enumerate() {
        button.wait_for_rising_edge().await;
        println!("running Test {}", i + 1);
        sender.send(i + 1);
        test(&mut safe_a, &mut fire_a, &mut safe_b, &mut fire_b);
    }
    core::future::pending::<()>().await;
}

static TW: StaticCell<Watch<ThreadModeRawMutex, usize, 1>> = StaticCell::new();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    info!("Launching");
    
    let button = ExtiInput::new(p.PC13, p.EXTI13, Pull::Down);

    let safe_a = Output::new(p.PA10, Level::Low, Speed::Low);
    let fire_a = Output::new(p.PB3, Level::Low, Speed::Low);

    let safe_b = Output::new(p.PB5, Level::Low, Speed::Low);
    let fire_b = Output::new(p.PB4, Level::Low, Speed::Low);

    let adc_periph = Adc::new(p.ADC1);

    let temp_watch = Watch::<ThreadModeRawMutex, i16, 1>::new();
    let out_a_watch = Watch::<ThreadModeRawMutex, i16, 1>::new();
    let out_b_watch = Watch::<ThreadModeRawMutex, i16, 1>::new();
    let current_test_watch = TW.init(Watch::new());
    
    let out_a_channel = AdcCtrlChannel::new(
        p.PA0.degrade_adc(),
        out_a_watch.sender().as_dyn(),
        adc::conversion::calculate_voltage_10mv
    );

    let out_b_channel = AdcCtrlChannel::new(
        p.PA1.degrade_adc(),
        out_b_watch.sender().as_dyn(),
        adc::conversion::calculate_voltage_10mv
    );

    let mut adc: AdcCtrl<'_, '_, _, 3> = AdcCtrl::new(adc_periph, p.DMA1_CH1, temp_watch.sender().as_dyn(), [out_a_channel, out_b_channel]);
    
    current_test_watch.sender().send(0);
    spawner.must_spawn(run_tasks(current_test_watch.sender(), button, safe_a, fire_a, safe_b, fire_b));

    let mut temp_receiver = temp_watch.receiver().unwrap();
    let mut out_a_receiver = out_a_watch.receiver().unwrap();
    let mut out_b_receiver = out_b_watch.receiver().unwrap();
    let mut current_test_receiver = current_test_watch.receiver().unwrap();

    loop {
        adc.run().await;
        println!("temp: {}, out A: {}, out B: {}, test: {}",
            temp_receiver.get().await,
            out_a_receiver.get().await,
            out_b_receiver.get().await,
            current_test_receiver.get().await
        );
        Timer::after(Duration::from_millis(100)).await;
    }
}
