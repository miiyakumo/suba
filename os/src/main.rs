//! os crate — RISC-V 硬件后端
//!
//! 这是 suba 内核的二进制入口，负责：
//! 1. entry.S: 设置栈、清零 BSS、跳转 rust_main
//! 2. rust_main: 初始化内核子系统，进入主循环

#![no_std]
#![no_main]

core::arch::global_asm!(include_str!("entry.S"));

mod power;
mod debug_console;

use core::arch::asm;

/// 向 QEMU UART (NS16550A) 输出一个字符
///
/// UART0 基地址 0x1000_0000，THR 偏移 0，LSR 偏移 5
/// LSR bit 5 (THR Empty) 为 1 时才能写入
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

/// 向 UART 输出字符串
pub fn uart_puts(s: &str) {
    for b in s.bytes() {
        uart_putchar(b);
    }
}

/// 内核 Rust 入口
///
/// 由 entry.S 调用，此时：
/// - 栈已设置（sp → boot_stack_top）
/// - BSS 已清零（汇编级）
/// - 中断已关闭（OpenSBI 默认）
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    uart_puts("Hello, suba!\n");
    power::shutdown(false);
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Phase 2 后续 feature 会添加 panic 输出
    power::shutdown(false);
}
