//! # Task — 任务控制块
//!
//! 定义任务的数据结构和状态机。
//!
//! ## 教学概念
//! - **Task Control Block (TCB)**：操作系统中每个"执行流"的身份证
//! - **TaskState 状态机**：任务在其生命周期中经历的状态转换
//! - **PID**：进程标识符，内核中的唯一 ID

use crate::arch::Context;

/// 任务状态枚举
///
/// ## 状态机转换
/// ```text
///     ┌──────┐
///     │ Ready │ ←── 新任务创建
///     └──┬───┘
///        │ 被调度器选中
///        ▼
///     ┌──────┐
///     │Running│
///     └──┬───┘
///        │
///   ┌────┼────┐
///   │    │    │
///   ▼    │    ▼
/// ┌───┐  │  ┌─────┐
/// │Wait│  │  │Exited│
/// └─┬─┘  │  └─────┘
///   │    │
///   └────┘→ 回到 Ready
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 就绪态：等待被调度
    Ready,
    /// 运行态：正在 CPU 上执行
    Running,
    /// 等待态：等待某个事件（如 I/O 完成）
    Waiting,
    /// 退出态：任务已结束
    Exited,
}

/// 任务控制块 (TCB)。
///
/// 包含一个任务的所有内核管理信息。
/// 这是操作系统中最核心的数据结构之一。
pub struct Task {
    /// 任务 ID
    pub pid: usize,
    /// 任务状态
    pub state: TaskState,
    /// 任务上下文（用于上下文切换）
    pub context: Context,
    /// 内核栈指针
    pub kstack_top: usize,
    /// 用户栈指针
    pub ustack_top: usize,
    /// 退出码
    pub exit_code: i32,
}

impl Task {
    /// 创建新的任务
    pub fn new(pid: usize, entry: usize, kstack_top: usize, ustack_top: usize) -> Self {
        let mut context = Context::zero_init();
        context.set_init_context(entry, kstack_top);

        Self {
            pid,
            state: TaskState::Ready,
            context,
            kstack_top,
            ustack_top,
            exit_code: 0,
        }
    }

    /// 获取上下文指针
    pub fn context_ptr(&mut self) -> *mut Context {
        &mut self.context as *mut Context
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_creation() {
        let task = Task::new(1, 0x8020_0000, 0x8040_0000, 0x8060_0000);
        assert_eq!(task.pid, 1);
        assert_eq!(task.state, TaskState::Ready);
        assert_eq!(task.context.ra, 0x8020_0000);
        assert_eq!(task.context.sp, 0x8040_0000);
        assert_eq!(task.exit_code, 0);
    }

    #[test]
    fn task_state_transitions() {
        let mut task = Task::new(1, 0, 0, 0);
        assert_eq!(task.state, TaskState::Ready);

        task.state = TaskState::Running;
        assert_eq!(task.state, TaskState::Running);

        task.state = TaskState::Waiting;
        assert_eq!(task.state, TaskState::Waiting);

        task.state = TaskState::Exited;
        assert_eq!(task.state, TaskState::Exited);
    }
}
