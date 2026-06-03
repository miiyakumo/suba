//! # TaskManager — 任务管理器
//!
//! 参考实现：管理所有任务的创建、查找和回收。
//!
//! ## 教学概念
//! - **任务管理器**：内核中负责管理所有任务生命周期的组件
//! - **PID 分配**：自动递增分配唯一标识符
//! - **任务查找**：通过 PID 快速定位任务

use alloc::collections::BTreeMap;
use alloc::sync::Arc;
use spin::Mutex;

use super::task::{Task, TaskState};
use super::scheduler::TaskHandle;

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
    pub fn create_task(&mut self, entry: usize, kstack_top: usize, ustack_top: usize) -> TaskHandle {
        let pid = self.next_pid;
        self.next_pid += 1;
        let task = Arc::new(Mutex::new(Task::new(pid, entry, kstack_top, ustack_top)));
        self.tasks.insert(pid, task.clone());
        task
    }

    /// 根据 PID 查找任务
    pub fn get_task(&self, pid: usize) -> Option<TaskHandle> {
        self.tasks.get(&pid).cloned()
    }

    /// 标记任务退出
    ///
    /// 将任务状态设为 Exited，但不立即移除。
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
