//! # 任务管理 (task)
//!
//! 本模块实现了多任务调度的核心逻辑。
//!
//! ## 教学概念
//! - **Task Control Block (TCB)**：每个任务的所有信息集中在一个结构体中
//! - **TaskState**：任务状态机（Ready → Running → Waiting → Exited）
//! - **上下文切换**：保存/恢复 callee-saved 寄存器，实现任务切换
//! - **Round-Robin 调度**：最简单的调度算法——轮流执行
//!
//! ## 教学故事
//! "有了多个任务之后，如何在它们之间切换？"
//! 答案：保存当前任务的寄存器，恢复下一个任务的寄存器——就这么简单。

pub mod task;
pub mod scheduler;

pub use task::{Task, TaskState};
pub use scheduler::{RoundRobinScheduler, TaskHandle};

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Mutex;

// TODO: 实现 TaskManager
// 参考 .solution/4.8-task-manager.rs
//
// 提示：
// - 使用 BTreeMap<usize, TaskHandle> 存储任务
// - create_task() 自动分配 PID
// - get_task() 按 PID 查找
// - exit_task() 设置任务状态为 Exited

/// 任务管理器。
///
/// 管理所有任务的创建、查找和回收。
/// 使用 BTreeMap 以 PID 为 key 存储任务。
pub struct TaskManager {
    /// PID → 任务句柄的映射
    tasks: BTreeMap<usize, TaskHandle>,
    /// 下一个分配的 PID
    next_pid: usize,
}

impl TaskManager {
    /// 创建新的任务管理器
    pub const fn new() -> Self {
        Self {
            tasks: BTreeMap::new(),
            next_pid: 1,
        }
    }

    /// 创建新任务并返回其句柄
    ///
    /// 自动分配 PID，将任务加入管理器。
    /// 创建的是内核任务（satp = 0，使用内核页表）。
    pub fn create_task(&mut self, entry: usize, kstack_top: usize, ustack_top: usize) -> TaskHandle {
        let pid = self.next_pid;
        self.next_pid += 1;
        let task = Arc::new(Mutex::new(Task::new(pid, entry, kstack_top, ustack_top)));
        self.tasks.insert(pid, task.clone());
        task
    }

    /// 创建用户任务并返回其句柄
    ///
    /// 与 `create_task` 类似，但额外设置独立的页表和用户入口。
    /// 用户任务在上下文切换时会切换到自己的地址空间。
    ///
    /// # 参数
    /// - `entry`: 内核线程入口（trampoline 函数）
    /// - `kstack_top`: 内核栈顶地址
    /// - `ustack_top`: 用户栈顶地址
    /// - `page_table_root`: 页表根物理页号（来自 `UserAddrSpace::root_ppn()`）
    /// - `user_entry`: 用户程序入口地址（sret 目标）
    pub fn create_user_task(
        &mut self,
        entry: usize,
        kstack_top: usize,
        ustack_top: usize,
        page_table_root: usize,
        user_entry: usize,
    ) -> TaskHandle {
        let pid = self.next_pid;
        self.next_pid += 1;
        let mut task = Task::new(pid, entry, kstack_top, ustack_top);
        task.set_page_table_root(page_table_root);
        task.user_entry = user_entry;
        let task = Arc::new(Mutex::new(task));
        self.tasks.insert(pid, task.clone());
        task
    }

    /// 根据 PID 查找任务
    pub fn get_task(&self, pid: usize) -> Option<TaskHandle> {
        self.tasks.get(&pid).cloned()
    }

    /// 标记任务退出
    ///
    /// 将任务状态设为 Exited，但不立即移除（由回收机制处理）。
    pub fn exit_task(&mut self, pid: usize, exit_code: i32) -> bool {
        if let Some(task) = self.tasks.get(&pid) {
            let mut t = task.lock();
            t.state = TaskState::Exited;
            t.exit_code = exit_code;
            true
        } else {
            false
        }
    }

    /// 任务总数
    pub fn len(&self) -> usize {
        self.tasks.len()
    }

    /// 是否没有任务
    pub fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_manager_create() {
        let mut tm = TaskManager::new();
        assert!(tm.is_empty());

        let task = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        assert_eq!(task.lock().pid, 1);
        assert_eq!(tm.len(), 1);
    }

    #[test]
    fn task_manager_get() {
        let mut tm = TaskManager::new();
        let task = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        let pid = task.lock().pid;

        let found = tm.get_task(pid);
        assert!(found.is_some());
        assert_eq!(found.unwrap().lock().pid, pid);

        assert!(tm.get_task(999).is_none());
    }

    #[test]
    fn task_manager_exit() {
        let mut tm = TaskManager::new();
        let task = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        let pid = task.lock().pid;

        assert!(tm.exit_task(pid, 0));
        assert_eq!(task.lock().state, TaskState::Exited);
        assert_eq!(task.lock().exit_code, 0);

        // 不存在的 PID
        assert!(!tm.exit_task(999, 1));
    }

    #[test]
    fn task_manager_multiple_tasks() {
        let mut tm = TaskManager::new();
        let t1 = tm.create_task(0x8020_0000, 0x8040_0000, 0);
        let t2 = tm.create_task(0x8020_0000, 0x8050_0000, 0);
        let t3 = tm.create_task(0x8020_0000, 0x8060_0000, 0);

        assert_eq!(tm.len(), 3);
        assert_eq!(t1.lock().pid, 1);
        assert_eq!(t2.lock().pid, 2);
        assert_eq!(t3.lock().pid, 3);
    }
}
