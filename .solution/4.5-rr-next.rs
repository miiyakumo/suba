//! # RoundRobinScheduler::next 实现
//!
//! 参考实现：Round-Robin 调度的核心算法。

use alloc::collections::VecDeque;
use alloc::sync::Arc;
use spin::Mutex;

use super::task::{Task, TaskState};

/// 任务句柄类型
pub type TaskHandle = Arc<Mutex<Task>>;

/// Round-Robin 调度器。
pub struct RoundRobinScheduler {
    queue: VecDeque<TaskHandle>,
}

impl RoundRobinScheduler {
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

    /// 选择下一个要运行的任务
    ///
    /// ## Round-Robin 算法
    /// 1. 从队头取出任务
    /// 2. 如果是 Ready 状态，返回该任务
    /// 3. 如果不是 Ready 状态，放回队尾并尝试下一个
    /// 4. 遍历完所有任务仍无就绪任务则返回 None
    ///
    /// ## 为什么用 try_lock？
    /// 在真实内核中，调度器运行在中断上下文。
    /// 如果任务锁已被持有（比如任务正在被另一个 CPU 操作），
    /// try_lock 返回 None 而不是死锁。这是安全的做法。
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
