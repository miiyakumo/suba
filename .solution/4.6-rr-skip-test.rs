//! # 调度器测试 — 跳过退出任务和 Round-Robin 顺序
//!
//! 参考实现：完整的调度器测试用例。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::{Task, TaskState, RoundRobinScheduler};
    use alloc::sync::Arc;
    use spin::Mutex;

    /// 测试：调度器正确跳过 Exited 状态的任务
    ///
    /// 当所有任务都是 Exited 状态时，next() 应返回 None。
    #[test]
    fn scheduler_skips_exited_task() {
        let mut sched = RoundRobinScheduler::new();
        let task = Arc::new(Mutex::new(Task::new(1, 0, 0, 0)));
        task.lock().state = TaskState::Exited;
        sched.enqueue(task);

        let next = sched.next();
        assert!(next.is_none());
    }

    /// 测试：Round-Robin 顺序正确
    ///
    /// 多个 Ready 任务应按 FIFO 顺序依次返回。
    #[test]
    fn scheduler_round_robin_order() {
        let mut sched = RoundRobinScheduler::new();
        let t1 = Arc::new(Mutex::new(Task::new(1, 0, 0, 0)));
        let t2 = Arc::new(Mutex::new(Task::new(2, 0, 0, 0)));
        let t3 = Arc::new(Mutex::new(Task::new(3, 0, 0, 0)));

        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());
        sched.enqueue(t3.clone());

        // 依次取出应该是 1, 2, 3
        assert_eq!(sched.next().unwrap().lock().pid, 1);
        assert_eq!(sched.next().unwrap().lock().pid, 2);
        assert_eq!(sched.next().unwrap().lock().pid, 3);
    }
}
