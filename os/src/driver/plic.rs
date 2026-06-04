//! PLIC (Platform-Level Interrupt Controller) 驱动
//!
//! 本模块实现 RISC-V PLIC 中断控制器的初始化和管理。
//! PLIC 负责将外部设备中断（如 UART）路由到 CPU 的 S-mode。
//!
//! ## 教学概念：PLIC 架构
//!
//! RISC-V 的中断分为两类：
//! - **本地中断**（CLINT）：时钟中断、软件中断，直接连接到每个 hart
//! - **外部中断**（PLIC）：来自设备（UART、网卡等），需要 PLIC 路由
//!
//! ```text
//! 设备 (UART, 网卡, ...)
//!   │ IRQ 线
//!   ▼
//! PLIC (Platform-Level Interrupt Controller)
//!   │ 根据优先级选择最高优先级中断
//!   │ 通知目标 hart 的 S-mode
//!   ▼
//! CPU (Supervisor External Interrupt, scause = 9)
//! ```
//!
//! ## QEMU virt PLIC 内存布局
//!
//! ```text
//! 基地址: 0x0C00_0000
//!
//! +0x000000  Priority[source]     (每个中断源的优先级，32-bit)
//!            source 0 保留，source 1~1023 可用
//!
//! +0x001000  Pending[0:31]        (中断挂起位，只读，32-bit)
//! +0x001004  Pending[32:63]
//! ...
//!
//! +0x002000  Enable[context=0]    (中断使能位，每个 context 一组)
//! +0x002080  Enable[context=1]    (S-mode hart 0)
//! ...
//!
//! +0x200000  Threshold[context=0] (优先级阈值)
//! +0x200004  Claim[context=0]     (认领/完成寄存器)
//! +0x201000  Threshold[context=1] (S-mode hart 0)
//! +0x201004  Claim[context=1]     (S-mode hart 0)
//! ```
//!
//! ## 教学概念：PLIC 中断流程
//!
//! ```text
//! 1. 设备触发中断（如 UART 收到数据）
//! 2. PLIC 记录 Pending 位
//! 3. PLIC 比较优先级和 Threshold
//! 4. 如果通过，通知目标 context（S-mode hart 0）
//! 5. CPU 收到 Supervisor external interrupt (scause = 9)
//! 6. 内核读取 Claim 寄存器获取中断源 ID
//! 7. 内核处理中断
//! 8. 内核写入 Complete 寄存器通知 PLIC 处理完毕
//! ```

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// PLIC 硬件常量
// ============================================================================

/// QEMU virt 机器 PLIC MMIO 基地址
pub const PLIC_BASE: usize = 0x0C00_0000;

/// UART0 中断源 ID（QEMU virt 设备树定义）
///
/// QEMU virt 机器的中断源分配：
/// - source 0: 保留（不存在的中断源）
/// - source 1~9: 其他设备
/// - source 10: UART0 (NS16550A)
/// - source 11~1023: 其他设备
pub const UART0_IRQ: usize = 10;

/// S-mode hart 0 的 context ID
///
/// PLIC 为每个 hart 的每个特权级分配独立的 context：
/// - context 0: hart 0 M-mode
/// - context 1: hart 0 S-mode
/// - context 2: hart 1 M-mode
/// - context 3: hart 1 S-mode
///
/// 我们需要配置 S-mode context，因为内核运行在 S-mode。
pub const S_MODE_CONTEXT: usize = 1;

// ============================================================================
// PLIC 寄存器偏移
// ============================================================================

/// 优先级寄存器偏移
/// Priority[source] = PLIC_BASE + source * 4
/// 值范围 0~7，0 表示禁用，7 最高优先级
const PRIORITY_OFFSET: usize = 0x000000;

/// Pending 寄存器偏移
/// Pending[group] = PLIC_BASE + 0x1000 + group * 4
/// 每个 bit 对应一个中断源（只读）
const PENDING_OFFSET: usize = 0x001000;

/// Enable 寄存器偏移
/// Enable[context][group] = PLIC_BASE + 0x2000 + context * 0x80 + group * 4
/// 每个 bit 对应一个中断源的使能状态
const ENABLE_OFFSET: usize = 0x002000;

/// Threshold 寄存器偏移
/// Threshold[context] = PLIC_BASE + 0x200000 + context * 0x1000
/// 只有优先级 > threshold 的中断才会被传递
const THRESHOLD_OFFSET: usize = 0x200000;

/// Claim/Complete 寄存器偏移
/// Claim[context] = PLIC_BASE + 0x200004 + context * 0x1000
/// 读取：认领中断（返回中断源 ID）
/// 写入：完成中断（通知 PLIC 处理完毕）
const CLAIM_OFFSET: usize = 0x200004;

// ============================================================================
// PLIC 寄存器访问
// ============================================================================

/// 设置中断源的优先级
///
/// 优先级范围 0~7：
/// - 0: 禁用（不会传递到任何 context）
/// - 1~7: 数值越大优先级越高
///
/// ## 教学概念：中断优先级
///
/// 当多个中断同时挂起时，PLIC 选择优先级最高的传递。
/// 如果优先级相同，选择中断源 ID 最小的。
fn set_priority(source: usize, priority: u32) {
    let addr = PLIC_BASE + PRIORITY_OFFSET + source * 4;
    // SAFETY: 写入 PLIC 优先级寄存器，source 在有效范围内
    unsafe {
        write_volatile(addr as *mut u32, priority);
    }
}

/// 使能指定 context 的中断源
///
/// 在 Enable 寄存器中设置对应 bit，允许该中断源传递到指定 context。
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

/// 设置指定 context 的优先级阈值
///
/// 只有优先级 > threshold 的中断才会被传递到该 context。
/// 设为 0 表示允许所有非零优先级的中断。
fn set_threshold(context: usize, threshold: u32) {
    let addr = PLIC_BASE + THRESHOLD_OFFSET + context * 0x1000;
    // SAFETY: 写入 PLIC 阈值寄存器
    unsafe {
        write_volatile(addr as *mut u32, threshold);
    }
}

/// 认领中断（读取 Claim 寄存器）
///
/// 返回中断源 ID。如果返回 0，表示没有待处理的中断。
///
/// ## 教学概念：Claim/Complete 机制
///
/// PLIC 使用 "Claim-Complete" 模式处理中断：
/// 1. Claim（读取）：获取最高优先级的挂起中断，PLIC 自动清除 Pending 位
/// 2. Complete（写入）：通知 PLIC 中断处理完毕
///
/// 这保证了中断不会被重复传递，直到处理完毕。
pub fn claim(context: usize) -> u32 {
    let addr = PLIC_BASE + CLAIM_OFFSET + context * 0x1000;
    // SAFETY: 读取 PLIC Claim 寄存器
    unsafe { read_volatile(addr as *const u32) }
}

/// 完成中断（写入 Complete 寄存器）
///
/// 通知 PLIC 指定中断源已处理完毕。
/// 写入的值是中断源 ID（与 claim 返回值相同）。
pub fn complete(context: usize, source: u32) {
    let addr = PLIC_BASE + CLAIM_OFFSET + context * 0x1000;
    // SAFETY: 写入 PLIC Complete 寄存器
    unsafe {
        write_volatile(addr as *mut u32, source);
    }
}

// ============================================================================
// PLIC 初始化
// ============================================================================

/// 初始化 PLIC
///
/// 配置 PLIC 使 UART0 中断能传递到 S-mode。
///
/// ## 教学概念：PLIC 初始化序列
///
/// ```text
/// 1. 设置 UART0 中断源优先级（> 0）
/// 2. 设置 S-mode context 的阈值为 0（允许所有优先级）
/// 3. 在 S-mode context 使能 UART0 中断
/// ```
///
/// 初始化后，当 UART0 触发中断时：
/// - PLIC 通知 CPU（Supervisor external interrupt）
/// - CPU 读取 scause = (1<<63) | 9
/// - 内核调用 claim() 获取中断源 ID
/// - 内核处理中断
/// - 内核调用 complete() 通知 PLIC
pub fn init() {
    // 设置 UART0 中断优先级为 1（最低非零优先级）
    // 优先级 0 表示禁用，1~7 表示不同优先级
    set_priority(UART0_IRQ, 1);

    // 设置 S-mode context 的优先级阈值为 0
    // 允许所有优先级 > 0 的中断传递
    set_threshold(S_MODE_CONTEXT, 0);

    // 在 S-mode context 使能 UART0 中断
    enable_irq(S_MODE_CONTEXT, UART0_IRQ);
}

/// 处理外部中断
///
/// 由 trap_handler 在 scause == 9 时调用。
/// 读取 Claim 寄存器获取中断源 ID，分发到对应的设备处理函数。
///
/// ## 教学概念：外部中断处理流程
///
/// ```text
/// scause = (1<<63) | 9  (Supervisor external interrupt)
///   → claim() 获取中断源 ID
///   → 根据 ID 分发到设备驱动
///   → 设备驱动处理中断
///   → complete() 通知 PLIC 处理完毕
/// ```
pub fn handle_external_interrupt() {
    // 认领中断：读取 Claim 寄存器
    let source = claim(S_MODE_CONTEXT);

    if source == 0 {
        // 没有待处理的中断（可能是中断已被其他 hart 处理）
        return;
    }

    // 根据中断源 ID 分发
    match source as usize {
        UART0_IRQ => {
            // UART0 中断：调用 UART 的中断处理函数
            crate::driver::uart::UartConsole::handle_interrupt();
        }
        _ => {
            // 未知中断源，忽略
        }
    }

    // 完成中断：通知 PLIC 处理完毕
    complete(S_MODE_CONTEXT, source);
}
