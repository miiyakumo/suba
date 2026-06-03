//! # Mock 上下文切换
//!
//! 参考实现：Mock 环境下的上下文切换接口设计。
//!
//! ## 设计说明
//! Mock 环境下 context_switch 是 no-op（不需要真正切换寄存器），
//! 但需要设计 switch_task() 函数的接口，将调度器和上下文切换连接起来。
//!
//! ## 流程
//! 1. 从调度器获取下一个任务
//! 2. 保存当前任务上下文
//! 3. 恢复下一个任务的上下文（Mock 下是 no-op）
//! 4. 更新任务状态

use crate::arch::{Arch, Context};
use super::scheduler::{RoundRobinScheduler, TaskHandle};
use super::task::TaskState;

/// 执行一次任务切换
///
/// 从调度器取出下一个就绪任务，执行上下文切换。
/// 在 Mock 环境下，context_switch 是 no-op，但接口设计与真实硬件一致。
///
/// # Safety
///
/// 调用者必须确保 current_task 指向有效的任务上下文。
pub unsafe fn switch_task<A: Arch>(
    scheduler: &mut RoundRobinScheduler,
    current_task: &mut TaskHandle,
) -> bool {
    let next_task = match scheduler.next() {
        Some(task) => task,
        None => return false,
    };

    // 更新状态
    {
        let mut current = current_task.lock();
        current.state = TaskState::Ready;
    }
    {
        let mut next = next_task.lock();
        next.state = TaskState::Running;
    }

    // 执行上下文切换
    let old_ctx = current_task.lock().context_ptr();
    let new_ctx = next_task.lock() as *const _ as *const Context;
    // SAFETY: context_ptr 返回有效指针, new_ctx 指向被调度任务的上下文
    unsafe { A::context_switch(old_ctx, new_ctx) };

    // 更新当前任务引用
    *current_task = next_task;
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::mock::MockArch;
    use crate::task::Task;
    use alloc::sync::Arc;
    use spin::Mutex;

    /// 测试：Mock 上下文切换不 panic
    #[test]
    fn mock_context_switch_noop() {
        let mut sched = RoundRobinScheduler::new();
        let t1 = Arc::new(Mutex::new(Task::new(1, 0, 0, 0)));
        let t2 = Arc::new(Mutex::new(Task::new(2, 0, 0, 0)));

        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());

        let mut current = t1.clone();
        current.lock().state = TaskState::Running;

        // Mock 下 context_switch 是 no-op，不应 panic
        let switched = unsafe { switch_task::<MockArch>(&mut sched, &mut current) };
        assert!(switched);
        assert_eq!(current.lock().pid, 2);
    }
}
