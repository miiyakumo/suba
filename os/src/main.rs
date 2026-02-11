#![no_std]
#![no_main]

core::arch::global_asm!(include_str!("entry.S"));

mod power;
mod debug_console;

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    clear_bss();
    eprintln!("Hello, world!");
    power::shutdown(false);
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    eprintln!("Panic occurred: {}", info);
    power::shutdown(false);
}

fn clear_bss() {
    unsafe extern "C" {
        fn sbss();
        fn ebss();
    }

    (sbss as usize..ebss as usize).for_each(|a| {
        unsafe { (a as *mut u8).write_volatile(0) }
    });
}
