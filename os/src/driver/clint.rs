//! CLINT (Core Local Interrupter) 时钟驱动
//!
//! 本模块提供 RISC-V 时钟中断的初始化和管理。
//! CLINT 是 RISC-V 平台的本地中断控制器，负责：
//! - 时钟中断（mtime/mtimecmp）
//! - 软件中断（msip）
//! - 核间中断（IPI）
//!
//! ## 教学概念：RISC-V 时钟机制
//!
//! RISC-V 使用两个寄存器实现时钟中断：
//!
//! ```text
//! mtime    — 单调递增的计数器（硬件自动更新）
//! mtimecmp — 比较值（软件设置）
//!
//! 当 mtime >= mtimecmp 时，触发时钟中断。
//! ```
//!
//! 在 QEMU virt 机器中，mtime 频率约为 10 MHz。
//! 也就是说，mtime 每秒增加 10,000,000。
//!
//! ## S-mode 的限制
//!
//! mtime 和 mtimecmp 是 M-mode 寄存器，S-mode 不能直接访问。
//! 我们通过 SBI 调用（`sbi_rt::set_timer`）间接设置 mtimecmp。
//! OpenSBI（运行在 M-mode）会帮我们写入硬件寄存器。
//!
//! ## QEMU virt CLINT 内存布局
//!
//! ```text
//! 基地址: 0x0200_0000
//! +0x0000  msip[0]      (32-bit, hart 0 软件中断)
//! +0x0004  msip[1]      (32-bit, hart 1 软件中断)
//! ...
//! +0x4000  mtimecmp[0]  (64-bit, hart 0 时钟比较器)
//! +0x4008  mtimecmp[1]  (64-bit, hart 1 时钟比较器)
//! ...
//! +0xBFF8  mtime        (64-bit, 全局计数器)
//! ```
//!
// 允许 dead_code：clint 常量和部分函数在后续 feature (9.7+) 中使用
#![allow(dead_code)]

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// CLINT 硬件常量
// ============================================================================

/// QEMU virt 机器 CLINT MMIO 基地址
///
/// CLINT (Core Local Interrupter) 管理每个 hart 的本地中断：
/// 时钟中断、软件中断和核间中断（IPI）。
#[allow(dead_code)]
pub const CLINT_BASE: usize = 0x0200_0000;

/// mtimecmp 寄存器偏移（相对于 CLINT_BASE）
///
/// mtimecmp[hart_id] = CLINT_BASE + 0x4000 + 8 * hart_id
/// 每个 hart 有独立的 64-bit 比较值。
#[allow(dead_code)]
pub const MTIMECMP_OFFSET: usize = 0x4000;

/// mtime 寄存器偏移（相对于 CLINT_BASE）
///
/// 全局共享的 64-bit 单调递增计数器。
/// 在 QEMU 中频率约为 10 MHz。
#[allow(dead_code)]
pub const MTIME_OFFSET: usize = 0xBFF8;

// ============================================================================
// 时钟配置
// ============================================================================

/// 时钟中断间隔（mtime tick 数）
///
/// QEMU mtime 频率约 10 MHz，10_000_000 tick ≈ 1 秒。
/// 设置为 1_000_000 tick ≈ 100ms，提供合理的调度粒度。
pub const TIMER_INTERVAL: u64 = 1_000_000;

// ============================================================================
// 时钟 API
// ============================================================================

/// 读取 mtime CSR（当前时间）
///
/// ## 教学概念：time CSR
///
/// `time` 是 RISC-V 的只读 CSR，返回当前 mtime 值。
/// 在 S-mode 可以直接读取，无需 SBI 调用。
///
/// 注意：虽然 CLINT 的 mtime 是 M-mode MMIO 寄存器，
/// 但 RISC-V 规范提供了 `time` CSR 作为 S-mode 的读取接口。
#[inline]
pub fn get_time() -> u64 {
    let time: u64;
    // SAFETY: time 是只读 CSR，无副作用
    unsafe {
        asm!(
            "csrr {}, time",
            out(reg) time,
            options(nomem, nostack)
        );
    }
    time
}

/// 设置下一次时钟中断
///
/// 通过 SBI 调用 `set_timer` 设置 mtimecmp 寄存器。
/// 当 mtime >= mtimecmp 时，触发 Supervisor timer interrupt (scause = 5)。
///
/// ## 教学概念：SBI 时钟接口
///
/// S-mode 无法直接写入 mtimecmp（M-mode 寄存器）。
/// SBI 的 Timer Extension 提供了 `set_timer` 调用：
///
/// ```text
/// ecall(EID_TIMER, FID_SET_TIMER, stime_value, 0, 0)
/// ```
///
/// OpenSBI 收到后会写入当前 hart 的 mtimecmp 寄存器。
pub fn set_next_timer() {
    let current = get_time();
    sbi_rt::set_timer(current + TIMER_INTERVAL);
}

/// 初始化时钟中断
///
/// 设置第一次时钟中断，并使能 sie.STIE 位。
///
/// ## 教学概念：时钟初始化序列
///
/// 1. 调用 `set_next_timer()` 设置第一次中断
/// 2. 使能 `sie` CSR 的 STIE (Supervisor Timer Interrupt Enable) 位
/// 3. 中断到来时，trap_handler 调用 `set_next_timer()` 设置下一次
///
/// 这形成了周期性时钟中断，驱动任务调度和时间片管理。
#[allow(dead_code)]
pub fn init() {
    // 设置第一次时钟中断
    set_next_timer();

    // 使能 Supervisor timer interrupt
    // sie (Supervisor Interrupt Enable) 寄存器的 bit 5 是 STIE
    // (Supervisor Timer Interrupt Enable)
    //
    // SAFETY: 设置 sie.STIE 位，允许时钟中断传递到 S-mode
    unsafe {
        asm!(
            "csrs sie, {0}",
            in(reg) 1 << 5,
            options(nomem, nostack)
        );
    }
}

// ============================================================================
// 全局 Tick 计数器
// ============================================================================

/// 全局时钟中断计数器
///
/// 每次时钟中断递增一次，用于：
/// - 计算系统运行时间（ticks / TICKS_PER_SEC = 秒数）
/// - 驱动时间片轮转调度（每个任务分配 N 个 tick）
/// - 实现内核定时器（sleep、超时等）
///
/// ## 教学概念：为什么用 AtomicUsize？
///
/// 时钟中断可能在任意时刻打断正在执行的代码。
/// 如果用普通 `usize`，递增操作（load → add → store）可能被中断打断，
/// 导致计数丢失。`AtomicUsize` 保证操作的原子性。
static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

/// 每秒时钟中断次数
///
/// TIMER_INTERVAL = 1_000_000 tick，mtime 频率 10 MHz，
/// 所以 10_000_000 / 1_000_000 = 10 次/秒（每 100ms 一次）。
#[allow(dead_code)]
pub const TICKS_PER_SEC: usize = 10;

/// 获取当前 tick 计数
///
/// 返回自内核启动以来的时钟中断次数。
/// 用于时间片管理和系统运行时间计算。
#[allow(dead_code)]
#[inline]
pub fn get_ticks() -> usize {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// 处理时钟中断
///
/// 时钟中断处理的核心函数，由 trap_handler 在 scause == 5 时调用。
///
/// ## 教学概念：时钟中断处理流程
///
/// ```text
/// 硬件: mtime >= mtimecmp
///   → CPU 触发 Supervisor timer interrupt (scause = 5)
///   → trap_handler 分发到此处
///   → 1. 递增 tick 计数
///   → 2. 设置下一次中断（维持周期性）
///   → 3. 后续：检查是否需要调度（时间片用完）
/// ```
///
/// 这是"滴答驱动"（tick-driven）内核的基础：
/// 每个 tick 是内核感知时间流逝的最小单位。
pub fn handle_timer_interrupt() {
    // 递增全局 tick 计数
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

    // 设置下一次时钟中断（保持周期性）
    set_next_timer();

    // TODO: 时间片调度检查（后续 feature）
    // 当 tick 计数达到时间片阈值时，触发任务切换
}
