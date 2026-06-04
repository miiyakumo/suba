//! # Solution: 9.7 — PLIC 初始化和中断路由
//!
//! 完整的 PLIC 驱动实现参考。
//! 学生应参考此文件理解 PLIC 的初始化流程。
//!
//! ## 关键要点
//!
//! 1. PLIC 使用 context 而非 hart_id 来区分中断目标
//! 2. S-mode context = 2 × hart_id + 1
//! 3. 初始化三步：设置优先级 → 设置阈值 → 使能中断源
//! 4. 还需使能 CPU 的 sie.SEIE 位（外部中断总开关）

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// 硬件常量
// ============================================================================

/// PLIC MMIO 基地址 (QEMU virt)
pub const PLIC_BASE: usize = 0x0C00_0000;

/// UART0 在 PLIC 中的中断号
pub const UART0_IRQ: usize = 10;

/// S-mode context (hart 0): context = 2 * hart_id + 1
pub const S_MODE_CONTEXT: usize = 1;

// ============================================================================
// 寄存器偏移量
// ============================================================================

/// Priority[source] = base + source * 4
const PRIORITY_OFFSET: usize = 0x000000;
/// Enable[ctx][group] = base + 0x2000 + ctx * 0x80 + group * 4
const ENABLE_OFFSET: usize = 0x002000;
/// Threshold[ctx] = base + 0x200000 + ctx * 0x1000
const THRESHOLD_OFFSET: usize = 0x200000;
/// Claim/Complete[ctx] = base + 0x200004 + ctx * 0x1000
const CLAIM_OFFSET: usize = 0x200004;

// ============================================================================
// 寄存器访问
// ============================================================================

fn set_priority(source: usize, priority: u32) {
    let addr = PLIC_BASE + PRIORITY_OFFSET + source * 4;
    // SAFETY: 写入 PLIC 优先级寄存器
    unsafe { write_volatile(addr as *mut u32, priority); }
}

fn enable_irq(context: usize, source: usize) {
    let group = source / 32;
    let bit = source % 32;
    let addr = PLIC_BASE + ENABLE_OFFSET + context * 0x80 + group * 4;
    // SAFETY: 读取并修改 PLIC 使能寄存器
    unsafe {
        let old: u32 = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old | (1 << bit));
    }
}

fn set_threshold(context: usize, threshold: u32) {
    let addr = PLIC_BASE + THRESHOLD_OFFSET + context * 0x1000;
    // SAFETY: 写入 PLIC 阈值寄存器
    unsafe { write_volatile(addr as *mut u32, threshold); }
}

/// 认领中断（读取 Claim 寄存器）
pub fn claim(context: usize) -> u32 {
    let addr = PLIC_BASE + CLAIM_OFFSET + context * 0x1000;
    // SAFETY: 读取 PLIC Claim 寄存器
    unsafe { read_volatile(addr as *const u32) }
}

/// 完成中断（写入 Complete 寄存器）
pub fn complete(context: usize, source: u32) {
    let addr = PLIC_BASE + CLAIM_OFFSET + context * 0x1000;
    // SAFETY: 写入 PLIC Complete 寄存器
    unsafe { write_volatile(addr as *mut u32, source); }
}

// ============================================================================
// 初始化
// ============================================================================

/// 初始化 PLIC：
/// 1. 设置 UART0 优先级 = 1（非零即可被路由）
/// 2. 设置 S-mode 阈值 = 0（接受所有优先级 > 0 的中断）
/// 3. 使能 S-mode context 的 UART0 中断
/// 4. 使能 CPU 的 sie.SEIE（外部中断总开关）
pub fn init() {
    // Step 1: 设置中断源优先级
    set_priority(UART0_IRQ, 1);

    // Step 2: 设置 S-mode context 阈值
    set_threshold(S_MODE_CONTEXT, 0);

    // Step 3: 使能 UART0 中断
    enable_irq(S_MODE_CONTEXT, UART0_IRQ);

    // Step 4: 使能 CPU 的 S-mode 外部中断
    // sie.SEIE = bit 9
    // SAFETY: 设置 sie.SEIE 位，允许外部中断传递到 S-mode
    unsafe {
        core::arch::asm!("csrs sie, {0}", in(reg) 1 << 9, options(nomem, nostack));
    }
}

// ============================================================================
// 中断处理
// ============================================================================

/// 处理外部中断（由 trap_handler 调用）
pub fn handle_external_interrupt() {
    let source = claim(S_MODE_CONTEXT);
    if source == 0 { return; }

    match source as usize {
        UART0_IRQ => {
            let uart = crate::driver::uart::Uart::new(crate::driver::uart::UART0_BASE);
            uart.handle_interrupt();
        }
        _ => {}
    }

    complete(S_MODE_CONTEXT, source);
}
