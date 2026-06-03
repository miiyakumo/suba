#![no_std]
#![no_main]

core::arch::global_asm!(include_str!("entry.S"));

mod power;
mod debug_console;
use core::arch::asm;

#[inline(always)]
pub fn uart_putchar(c: u8) {
    unsafe {
        asm!(
        "li t0, 0x10000000",   // UART base
        "1:",
        "lb t1, 5(t0)",        // LSR = base + 5
        "andi t1, t1, 0x20",   // THR empty?
        "beqz t1, 1b",         // 等待可发送
        "sb {0}, 0(t0)",       // 写入字符
        in(reg) c,
        out("t0") _,
        out("t1") _,
        );
    }
}

pub fn uart_puts(s: &str) {
    for b in s.bytes() {
        uart_putchar(b);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    clear_bss();
    uart_puts("Hello, world!");
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
