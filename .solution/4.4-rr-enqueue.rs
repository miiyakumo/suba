//! # RoundRobinScheduler — new 和 enqueue 实现
//!
//! 参考实现：构造函数和入队方法。

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
    /// 创建新的调度器
    ///
    /// 使用 const fn 使其可在编译期初始化。
    pub const fn new() -> Self {
        Self {
            queue: VecDeque::new(),
        }
    }

    /// 将任务加入就绪队列
    ///
    /// 新任务总是加入队尾（FIFO 语义）。
    pub fn enqueue(&mut self, task: TaskHandle) {
        self.queue.push_back(task);
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
