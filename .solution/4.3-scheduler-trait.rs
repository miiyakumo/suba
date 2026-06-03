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

/// 调度器接口 trait。
///
/// 抽象调度算法的核心操作：入队、出队、查询。
/// 不同的调度算法（Round-Robin、CFS、优先级调度）只需实现这个 trait。
pub trait Scheduler {
    /// 将任务加入就绪队列
    fn enqueue(&mut self, task: TaskHandle);
    /// 选择下一个要运行的任务
    fn next(&mut self) -> Option<TaskHandle>;
    /// 队列中的任务数量
    fn len(&self) -> usize;
    /// 队列是否为空
    fn is_empty(&self) -> bool;
}

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
    /// 创建新的调度器
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }
}

impl Scheduler for RoundRobinScheduler {
    fn enqueue(&mut self, task: TaskHandle) {
        self.queue.push_back(task);
    }

    /// Round-Robin 算法：
    /// 1. 从队头取出任务
    /// 2. 如果是就绪状态，返回该任务
    /// 3. 如果不是就绪状态，放回队尾并尝试下一个
    /// 4. 遍历完所有任务仍无就绪任务则返回 None
    fn next(&mut self) -> Option<TaskHandle> {
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

    fn len(&self) -> usize {
        self.queue.len()
    }

    fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }
}
