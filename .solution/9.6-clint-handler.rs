// .solution/9.6-clint-handler.rs — CLINT 时钟中断处理参考实现
//
// 本文件是 feature 9.6 的参考实现。
// 学生应在 os/src/driver/clint.rs 中实现时钟中断处理。
//
// ## 实现要点
//
// 1. 全局 tick 计数器（AtomicUsize）
// 2. get_ticks() — 读取当前 tick 数
// 3. handle_timer_interrupt() — 递增 tick + 设置下一次中断
// 4. trap_handler 调用 clint::handle_timer_interrupt()
//
// ## 教学概念：时钟中断驱动调度
//
// 时钟中断是操作系统实现"并发"的基础硬件机制：
// - 硬件周期性触发中断，打断正在执行的任务
// - 内核在中断处理中递增 tick 计数
// - 当任务用完时间片（N 个 tick），触发上下文切换
// - 这实现了"时间片轮转调度"——每个任务公平地获得 CPU 时间
//
// ## 为什么用 AtomicUsize？
//
// 时钟中断可能在任意时刻打断正在执行的代码。
// 普通 usize 的 load → add → store 不是原子的：
//   CPU: load(5) → 中断! → 其他代码读到 5 → add → store(6)
//   结果：中断中的读取看到了旧值
// AtomicUsize 保证整个操作是原子的。

use core::sync::atomic::{AtomicUsize, Ordering};

/// 全局时钟中断计数器
static TIMER_TICKS: AtomicUsize = AtomicUsize::new(0);

/// 每秒时钟中断次数
/// TIMER_INTERVAL = 1_000_000, mtime 频率 10 MHz → 10 次/秒
pub const TICKS_PER_SEC: usize = 10;

/// 获取当前 tick 计数
#[inline]
pub fn get_ticks() -> usize {
    TIMER_TICKS.load(Ordering::Relaxed)
}

/// 处理时钟中断（由 trap_handler 调用）
///
/// 1. 递增全局 tick 计数
/// 2. 设置下一次时钟中断（维持周期性）
/// 3. 后续 feature：检查时间片是否用完，触发调度
pub fn handle_timer_interrupt() {
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);
    set_next_timer();
    // TODO: 时间片调度检查（后续 feature）
}

// 注意：set_next_timer() 和 init() 保持不变（feature 9.5 已实现）
