//! # Round-Robin 调度器
//!
//! 实现最简单的调度算法——时间片轮转。
//!
//! ## 教学概念
//! - **调度器**：决定"下一个运行的任务是谁"
//! - **Round-Robin**：每个任务轮流执行一个时间片，用完就换下一个
//! - **就绪队列**：所有等待运行的任务排成一个队列
//!
//! ## 算法
//! 1. 新任务加入队尾
//! 2. 从队头取出下一个就绪任务
//! 3. 如果队头不是就绪状态（如 Waiting/Exited），跳过并尝试下一个
//! 4. 队列为空时返回 None

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use spin::Mutex;

use super::task::{Task, TaskState};

/// 任务句柄类型
pub type TaskHandle = Arc<Mutex<Task>>;

/// Round-Robin 调度器。
///
/// 使用 FIFO 队列管理就绪任务。
pub struct RoundRobinScheduler {
    queue: VecDeque<TaskHandle>,
}

impl Default for RoundRobinScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl RoundRobinScheduler {
    // TODO: 实现 RoundRobinScheduler::new 和 enqueue
    // 参考 .solution/4.4-rr-enqueue.rs
    //
    // 提示：
    // - new() 应创建一个空的 VecDeque 队列
    // - enqueue() 将任务加入队尾

    /// 创建新的调度器
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// 将任务加入就绪队列
    pub fn enqueue(&mut self, task: TaskHandle) {
        self.queue.push_back(task);
    }

    // TODO: 实现 RoundRobinScheduler::next
    // 参考 .solution/4.5-rr-next.rs
    //
    // 提示：
    // - 从队头取出任务，检查状态
    // - Ready 状态返回，其他状态放回队尾
    // - 用 try_lock 避免死锁

    /// 选择下一个要运行的任务
    ///
    /// Round-Robin 算法：
    /// 1. 从队头取出任务
    /// 2. 如果是就绪状态，返回该任务
    /// 3. 如果不是就绪状态，跳过并尝试下一个
    /// 4. 队列为空时返回 None
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Option<TaskHandle> {
        let len = self.queue.len();
        for _ in 0..len {
            let task = self.queue.pop_front()?;
            // SAFETY: try_lock 不会 panic，仅在已被持有时返回 None
            let state = task.try_lock().map(|t| t.state);
            match state {
                Some(TaskState::Ready) => return Some(task),
                _ => {
                    // 非就绪任务放回队尾，继续尝试下一个
                    self.queue.push_back(task);
                }
            }
        }
        None
    }

    /// 队列中的任务数量
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// 队列是否为空
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}

// TODO: 编写调度器测试
// 参考 .solution/4.6-rr-skip-test.rs
//
// 测试要点：
// - 跳过 Exited 状态的任务
// - Round-Robin 顺序正确性

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::TaskState;

    #[test]
    fn scheduler_enqueue_len() {
        let mut sched = RoundRobinScheduler::new();
        assert!(sched.is_empty());

        let task = Arc::new(Mutex::new(Task::new(1, 0x8020_0000, 0x8040_0000, 0)));
        sched.enqueue(task);
        assert_eq!(sched.len(), 1);
    }

    #[test]
    fn scheduler_next_returns_ready_task() {
        let mut sched = RoundRobinScheduler::new();
        let task = Arc::new(Mutex::new(Task::new(1, 0x8020_0000, 0x8040_0000, 0)));
        sched.enqueue(task.clone());

        let next = sched.next();
        assert!(next.is_some());
        let next = next.unwrap();
        assert_eq!(next.lock().pid, 1);
    }

    #[test]
    fn scheduler_skips_exited_task() {
        let mut sched = RoundRobinScheduler::new();
        let task = Arc::new(Mutex::new(Task::new(1, 0, 0, 0)));
        task.lock().state = TaskState::Exited;
        sched.enqueue(task);

        let next = sched.next();
        assert!(next.is_none());
    }

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

    // TODO: 编写集成测试 — TaskManager + Scheduler 联动
    // 参考 .solution/4.9-task-integration-test.rs
    //
    // 测试要点：
    // - TaskManager 创建任务 → 加入 Scheduler → 调度验证
    // - 混合状态（Ready/Exited）的调度行为

    #[test]
    fn task_manager_scheduler_integration() {
        use crate::task::TaskManager;

        let mut tm = TaskManager::new();
        let mut sched = RoundRobinScheduler::new();

        let t1 = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        let t2 = tm.create_task(0x8020_0000, 0x8050_0000, 0);

        sched.enqueue(t1.clone());
        sched.enqueue(t2.clone());

        assert_eq!(sched.next().unwrap().lock().pid, 1);
        assert_eq!(sched.next().unwrap().lock().pid, 2);

        // 退出任务 1 后调度器应跳过
        tm.exit_task(1, 0);
        assert!(sched.next().is_none());
    }
}
