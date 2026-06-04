//! # 6.9 参考实现：标准文件描述符初始化
//!
//! 核心改动：在 kernel/src/task/task.rs 中
//!
//! ## 关键变化
//! 1. Task 结构体添加 `fd_table: FdTable` 字段
//! 2. Task::new() 中预分配 stdin(0)、stdout(1)、stderr(2)
//! 3. 使用 RamFs 作为 Mock 后端的文件实现

use crate::arch::Context;
use crate::fs::FdTable;

pub struct Task {
    pub pid: usize,
    pub state: TaskState,
    pub context: Context,
    pub kstack_top: usize,
    pub ustack_top: usize,
    pub exit_code: i32,
    /// 文件描述符表 — 每个任务独立拥有
    pub fd_table: FdTable,
}

impl Task {
    pub fn new(pid: usize, entry: usize, kstack_top: usize, ustack_top: usize) -> Self {
        let mut context = Context::zero_init();
        context.set_init_context(entry, kstack_top);

        // 预分配标准文件描述符
        let mut fd_table = FdTable::new();
        fd_table.open(Box::new(RamFs::new())); // fd 0: stdin
        fd_table.open(Box::new(RamFs::new())); // fd 1: stdout
        fd_table.open(Box::new(RamFs::new())); // fd 2: stderr

        Self {
            pid,
            state: TaskState::Ready,
            context,
            kstack_top,
            ustack_top,
            exit_code: 0,
            fd_table,
        }
    }
}
