// .solution/9.5-clint-init.rs — CLINT 时钟初始化参考实现
//
// 本文件是 feature 9.5 的参考实现。
// 学生应在 os/src/driver/clint.rs 中实现 CLINT 时钟驱动。
//
// ## 实现要点
//
// 1. CLINT 硬件常量（基地址、寄存器偏移）
// 2. get_time() — 读取 time CSR
// 3. set_next_timer() — 通过 SBI 设置 mtimecmp
// 4. init() — 初始化时钟中断（设置第一次中断 + 使能 sie.STIE）
//
// ## 教学概念：RISC-V 时钟中断
//
// RISC-V 的时钟机制：
// - mtime: 硬件单调递增计数器（全局共享）
// - mtimecmp: 比较值（每个 hart 独立）
// - 当 mtime >= mtimecmp 时，触发 Supervisor timer interrupt (scause = 5)
//
// S-mode 无法直接访问 mtime/mtimecmp（M-mode 寄存器），
// 需要通过 SBI 调用间接设置。
//
// ## QEMU virt CLINT 内存布局
//
// 基地址: 0x0200_0000
// +0x0000  msip[0]      (32-bit, hart 0 软件中断)
// +0x4000  mtimecmp[0]  (64-bit, hart 0 时钟比较器)
// +0xBFF8  mtime        (64-bit, 全局计数器)
//
// QEMU mtime 频率约 10 MHz。
// TIMER_INTERVAL = 1_000_000 → 约 100ms 中断间隔。

use core::arch::asm;

pub const CLINT_BASE: usize = 0x0200_0000;
pub const MTIMECMP_OFFSET: usize = 0x4000;
pub const MTIME_OFFSET: usize = 0xBFF8;

pub const TIMER_INTERVAL: u64 = 1_000_000;

/// 读取 time CSR
pub fn get_time() -> u64 {
    let time: u64;
    unsafe {
        asm!("csrr {}, time", out(reg) time, options(nomem, nostack));
    }
    time
}

/// 设置下一次时钟中断（通过 SBI）
pub fn set_next_timer() {
    let current = get_time();
    sbi_rt::set_timer(current + TIMER_INTERVAL);
}

/// 初始化时钟中断
pub fn init() {
    set_next_timer();
    // 使能 sie.STIE (bit 5)
    unsafe {
        asm!("csrs sie, {0}", in(reg) 1 << 5, options(nomem, nostack));
    }
}
