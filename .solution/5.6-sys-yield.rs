//! # sys_yield 实现
//!
//! 参考实现：让出 CPU 系统调用。
//!
//! ## 教学概念
//! sys_yield 是协作式调度的核心——任务主动放弃 CPU，让其他任务运行。
//!
//! 在真实内核中，sys_yield 的工作流程：
//! 1. 获取当前任务的 TCB
//! 2. 将任务状态从 Running 改为 Ready
//! 3. 将任务放回调度队列尾部
//! 4. 调用 schedule() 切换到下一个任务
//!
//! 在 Mock 环境下，没有全局的 current_task 指针和调度器，
//! 因此只能返回 0 表示"让出成功"。
//!
//! ## 与 sys_exit 的区别
//! - sys_exit：任务终止，状态变为 Exited，不再被调度
//! - sys_yield：任务暂停，状态变为 Ready，等待下次被调度

/// 让出 CPU 系统调用。
///
/// 主动让出 CPU 给其他就绪任务。在协作式调度中，
/// 任务必须主动调用 sys_yield 才能让出 CPU。
///
/// # 返回值
/// 始终返回 0（表示让出成功）。
pub fn sys_yield() -> usize {
    // 在真实内核中：
    // 1. 获取当前任务
    //    let current = current_task();
    // 2. 将状态从 Running 改为 Ready
    //    current.lock().state = TaskState::Ready;
    // 3. 将任务放回调度队列
    //    scheduler.enqueue(current);
    // 4. 触发调度
    //    schedule();
    //
    // Mock 环境下，返回 0 表示让出成功
    0
}
