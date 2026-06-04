// .solution/9.7-plic-init.rs — PLIC 初始化和中断路由参考实现
//
// 本文件是 feature 9.7 的参考实现。
// 学生应在 os/src/driver/plic.rs 中实现 PLIC 驱动。
//
// ## 实现要点
//
// 1. PLIC 硬件常量（基地址、寄存器偏移、UART0 IRQ）
// 2. 优先级/阈值/使能寄存器访问函数
// 3. claim() 和 complete() — Claim-Complete 模式
// 4. init() — 配置 UART0 中断路由到 S-mode
// 5. handle_external_interrupt() — 中断分发
//
// ## 教学概念：PLIC 中断流程
//
// 1. 设备触发中断（UART 收到数据）
// 2. PLIC 记录 Pending 位
// 3. 比较优先级和 Threshold
// 4. 通知目标 context（S-mode hart 0）
// 5. CPU 收到 scause = (1<<63) | 9
// 6. 内核 claim() 获取中断源 ID
// 7. 内核处理中断
// 8. 内核 complete() 通知 PLIC

use core::ptr::{read_volatile, write_volatile};

pub const PLIC_BASE: usize = 0x0C00_0000;
pub const UART0_IRQ: usize = 10;
pub const S_MODE_CONTEXT: usize = 1;

fn set_priority(source: usize, priority: u32) {
    let addr = PLIC_BASE + source * 4;
    unsafe { write_volatile(addr as *mut u32, priority); }
}

fn enable_irq(context: usize, source: usize) {
    let group = source / 32;
    let bit = source % 32;
    let addr = PLIC_BASE + 0x2000 + context * 0x80 + group * 4;
    unsafe {
        let old: u32 = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old | (1 << bit));
    }
}

fn set_threshold(context: usize, threshold: u32) {
    let addr = PLIC_BASE + 0x200000 + context * 0x1000;
    unsafe { write_volatile(addr as *mut u32, threshold); }
}

pub fn claim(context: usize) -> u32 {
    let addr = PLIC_BASE + 0x200004 + context * 0x1000;
    unsafe { read_volatile(addr as *const u32) }
}

pub fn complete(context: usize, source: u32) {
    let addr = PLIC_BASE + 0x200004 + context * 0x1000;
    unsafe { write_volatile(addr as *mut u32, source); }
}

pub fn init() {
    set_priority(UART0_IRQ, 1);
    set_threshold(S_MODE_CONTEXT, 0);
    enable_irq(S_MODE_CONTEXT, UART0_IRQ);
}

pub fn handle_external_interrupt() {
    let source = claim(S_MODE_CONTEXT);
    if source == 0 { return; }
    match source as usize {
        UART0_IRQ => crate::driver::uart::UartConsole::handle_interrupt(),
        _ => {}
    }
    complete(S_MODE_CONTEXT, source);
}
