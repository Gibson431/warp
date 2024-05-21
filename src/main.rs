#![no_std]
#![no_main]

use defmt as _;
use embassy_executor::Spawner;
use embassy_rp::bind_interrupts;
use embassy_rp::gpio;
use embassy_rp::i2c::{self, Config};
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::usb::{self, Driver};
use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::{Duration, Ticker, Timer};
// use embedded_hal_async::i2c::I2c;
use gpio::{AnyPin, Level, Output};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::task]
async fn logger_task(driver: Driver<'static, USB>) {
    embassy_usb_logger::run!(1024, log::LevelFilter::Debug, driver);
}

type LedType = Mutex<ThreadModeRawMutex, Option<Output<'static>>>;
static LED: LedType = Mutex::new(None);

bind_interrupts!(struct Irqs {
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
});

#[embassy_executor::task(pool_size = 2)]
async fn heartbeat(name: &'static str, delay: Duration) {
    let mut ticker = Ticker::every(delay);
    loop {
        log::info!("{name} heartbeat");
        ticker.next().await;
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) -> ! {
    let p = embassy_rp::init(Default::default());
    let driver = Driver::new(p.USB, Irqs);
    spawner.spawn(logger_task(driver)).unwrap();
    Timer::after_secs(1).await;
    log::info!("program started");
    let led = Output::new(AnyPin::from(p.PIN_25), Level::High);

    // inner scope is so that once the mutex is written to, the MutexGuard is dropped, thus the
    // Mutex is released
    {
        *(LED.lock().await) = Some(led);
    }

    spawner
        .spawn(heartbeat("task1", Duration::from_secs(3)))
        .unwrap();
    spawner
        .spawn(heartbeat("task2", Duration::from_secs(5)))
        .unwrap();
    if let Err(_) = spawner.spawn(heartbeat("task3", Duration::from_secs(5))) {
        // SpawnError
        log::info!("Should error: Too many heartbeat tasks active");
    };

    let sda = p.PIN_14;
    let scl = p.PIN_15;
    let mut _i2c = i2c::I2c::new_async(p.I2C1, scl, sda, Irqs, Config::default());

    loop {
        {
            let mut led_unlocked = LED.lock().await;
            if let Some(pin_ref) = led_unlocked.as_mut() {
                pin_ref.toggle();
                log::info!("led toggled!");
            }
        }
        Timer::after_secs(1).await;
    }
}
