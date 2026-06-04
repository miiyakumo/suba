//! # sys_getpid 实现
//!
//! 参考实现：获取 PID 系统调用。
//!
//! ## 教学概念
//! getpid 是最简单的信息查询系统调用——它不修改任何状态，
//! 只是返回当前任务的标识符。
//!
//! 在真实内核中，sys_getpid 的工作流程：
//! 1. 获取当前任务的 TCB
//! 2. 读取 TCB 中的 pid 字段
//! 3. 返回 pid
//!
//! 在 Mock 环境下，没有全局 current_task，
//! 返回一个固定的 mock PID。

/// 获取 PID 系统调用。
///
/// 返回当前任务的进程标识符 (PID)。
///
/// # 返回值
/// 当前任务的 PID。
pub fn sys_getpid() -> usize {
    // 在真实内核中：
    //    let current = current_task();
    //    current.lock().pid
    //
    // Mock: 返回固定 PID
    1
}
