//! # 集成测试 — 任务创建和调度
//!
//! 参考实现：完整的任务生命周期集成测试。
//!
//! ## 测试流程
//! 1. 创建多个任务
//! 2. 加入调度器
//! 3. 依次取出验证顺序
//! 4. 验证状态转换

#[cfg(test)]
mod integration_tests {
    use crate::task::{Task, TaskState, RoundRobinScheduler, TaskManager};
    use alloc::sync::Arc;
    use spin::Mutex;

    /// 集成测试：创建 → 入队 → 调度 → 验证顺序
    #[test]
    fn task_lifecycle_integration() {
        let mut sched = RoundRobinScheduler::new();

        // 创建 3 个任务
        let t1 = Arc::new(Mutex::new(Task::new(1, 0x8020_0000, 0x8040_0000, 0)));
        let t2 = Arc::new(Mutex::new(Task::new(2, 0x8020_0000, 0x8050_0000, 0)));
        let t3 = Arc::new(Mutex::new(Task::new(3, 0x8020_0000, 0x8060_0000, 0)));

        // 加入调度器
        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());
        sched.enqueue(t3.clone());

        // Round-Robin 顺序验证
        assert_eq!(sched.next().unwrap().lock().pid, 1);
        assert_eq!(sched.next().unwrap().lock().pid, 2);
        assert_eq!(sched.next().unwrap().lock().pid, 3);
    }

    /// 集成测试：TaskManager + Scheduler 联动
    #[test]
    fn task_manager_scheduler_integration() {
        let mut tm = TaskManager::new();
        let mut sched = RoundRobinScheduler::new();

        // 通过 TaskManager 创建任务
        let t1 = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        let t2 = tm.create_task(0x8020_0000, 0x8050_0000, 0);

        // 加入调度器
        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());

        // 调度验证
        assert_eq!(sched.next().unwrap().lock().pid, 1);
        assert_eq!(sched.next().unwrap().lock().pid, 2);

        // 退出任务 1
        tm.exit_task(1, 0);
        assert_eq!(t1.lock().state, TaskState::Exited);

        // 调度器应跳过已退出的任务
        assert!(sched.next().is_none());
    }

    /// 集成测试：混合状态任务调度
    #[test]
    fn mixed_state_scheduling() {
        let mut sched = RoundRobinScheduler::new();

        let t1 = Arc::new(Mutex::new(Task::new(1, 0, 0, 0)));
        let t2 = Arc::new(Mutex::new(Task::new(2, 0, 0, 0)));
        let t3 = Arc::new(Mutex::new(Task::new(3, 0, 0, 0)));

        // t2 是 Exited 状态
        t2.lock().state = TaskState::Exited;

        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());
        sched.enqueue(t3.clone());

        // 应跳过 t2，返回 t1
        assert_eq!(sched.next().unwrap().lock().pid, 1);
        // t2 被跳过，返回 t3
        assert_eq!(sched.next().unwrap().lock().pid, 3);
    }
}
