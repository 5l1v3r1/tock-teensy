#![no_std]
#![no_main]
#![feature(asm,const_fn,lang_items)]

extern crate capsules;

#[macro_use(debug, static_init, register_bitfields, register_bitmasks)]
extern crate kernel;

#[allow(dead_code)]
extern crate mk66;

#[macro_use]
pub mod io;

#[allow(dead_code)]
mod tests;

#[allow(dead_code)]
mod spi;

#[allow(dead_code)]
mod components;

pub mod xconsole;

#[allow(dead_code)]
mod pins;

use components::*;

#[allow(unused)]
struct Teensy {
    xconsole: <XConsoleComponent as Component>::Output,
    gpio: <GpioComponent as Component>::Output,
    led: <LedComponent as Component>::Output,
    alarm: <AlarmComponent as Component>::Output,
    spi: <VirtualSpiComponent as Component>::Output,
    rng: <RngaComponent as Component>::Output,
    ipc: kernel::ipc::IPC,
}

impl kernel::Platform for Teensy {
    fn with_driver<F, R>(&self, driver_num: usize, f: F) -> R
        where F: FnOnce(Option<&kernel::Driver>) -> R
    {
        match driver_num {
            xconsole::DRIVER_NUM => f(Some(self.xconsole)),
            capsules::gpio::DRIVER_NUM => f(Some(self.gpio)),

            capsules::alarm::DRIVER_NUM => f(Some(self.alarm)),
            spi::DRIVER_NUM => f(Some(self.spi)),

            capsules::led::DRIVER_NUM => f(Some(self.led)),

            capsules::rng::DRIVER_NUM => f(Some(self.rng)),

            kernel::ipc::DRIVER_NUM => f(Some(&self.ipc)),
            _ => f(None),
        }
    }
}

#[link_section = ".flashconfig"]
#[no_mangle]
pub static FLASH_CONFIG_BYTES: [u8; 16] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
    0xFF, 0xFF, 0xFF, 0xFF, 0xDE, 0xF9, 0xFF, 0xFF,
];

#[no_mangle]
pub unsafe fn reset_handler() {
    // Disable the watchdog.
    mk66::wdog::stop();

    // Relocate the text and data segments.
    mk66::init();

    // Configure the system clock.
    mk66::clock::configure(120);

    // Enable the Port Control and Interrupt clocks.
    use mk66::sim::Clock;
    mk66::sim::clocks::PORTABCDE.enable();

    let (gpio_pins, led_pins) = pins::configure_all_pins();
    let gpio = GpioComponent::new()
                             .dependency(gpio_pins)
                             .finalize().unwrap();
    let led = LedComponent::new()
                           .dependency(led_pins)
                           .finalize().unwrap();
    let spi = VirtualSpiComponent::new().finalize().unwrap();
    let alarm = AlarmComponent::new().finalize().unwrap();
    let xconsole = XConsoleComponent::new().finalize().unwrap();
    let rng = RngaComponent::new().finalize().unwrap();

    let teensy = Teensy {
        xconsole: xconsole,
        gpio: gpio,
        led: led,
        alarm: alarm,
        spi: spi,
        rng: rng,
        ipc: kernel::ipc::IPC::new(),
    };

    let mut chip = mk66::chip::MK66::new();

    if tests::TEST {
        tests::test();
    }
    kernel::kernel_loop(&teensy, &mut chip, load_processes(), Some(&teensy.ipc));
}


unsafe fn load_processes() -> &'static mut [Option<&'static mut kernel::procs::Process<'static>>] {
    extern "C" {
        /// Beginning of the ROM region containing the app images.
        static _sapps: u8;
    }

    const NUM_PROCS: usize = 1;

    // Total memory allocated to the processes
    #[link_section = ".app_memory"]
    static mut APP_MEMORY: [u8; 1 << 17] = [0; 1 << 17];

    // How the kernel responds when a process faults
    const FAULT_RESPONSE: kernel::procs::FaultResponse = kernel::procs::FaultResponse::Panic;

    static mut PROCESSES: [Option<&'static mut kernel::procs::Process<'static>>; NUM_PROCS] = [None];

    kernel::procs::load_processes(
        &_sapps as *const u8,
        &mut APP_MEMORY,
        &mut PROCESSES,
        FAULT_RESPONSE,
    );

    &mut PROCESSES
}
