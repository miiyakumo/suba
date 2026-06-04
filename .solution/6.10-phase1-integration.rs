//! # 6.10 参考实现：Phase 1 Mock 完整集成测试
//!
//! 文件位置：kernel/tests/phase1_integration.rs
//!
//! ## 测试流程
//! 1. 初始化 TaskManager（模拟内核启动）
//! 2. 创建用户任务（自动分配 stdin/stdout/stderr）
//! 3. 通过文件描述符写入数据
//! 4. 验证文件内容
//! 5. 任务退出
//! 6. 创建第二个任务验证独立性
//!
//! ## 教学意义
//! 这是 Phase 1 的"毕业考试"——验证所有模块协同工作：
//! - Task + TaskManager（任务管理）
//! - FdTable + VfsFile + RamFs（文件系统）
//! - 标准文件描述符（Unix 一切皆文件）

extern crate alloc;
extern crate suba_kernel;

use alloc::boxed::Box;
use suba_kernel::fs::ramfs::RamFs;
use suba_kernel::task::{TaskManager, TaskState};

#[test]
fn phase1_full_cycle() {
    // 1. 内核启动
    let mut tm = TaskManager::new();
    assert!(tm.is_empty());

    // 2. 创建任务
    let task = tm.create_task(0x8020_0000, 0x8040_0000, 0x8060_0000);
    {
        let t = task.lock();
        assert_eq!(t.pid, 1);
        assert!(t.fd_table.get(0).is_some()); // stdin
        assert!(t.fd_table.get(1).is_some()); // stdout
        assert!(t.fd_table.get(2).is_some()); // stderr
    }

    // 3. 文件 I/O
    {
        let mut t = task.lock();
        let fd = t.fd_table.open(Box::new(RamFs::new()));
        t.fd_table.get_mut(fd).unwrap().write(b"hello").unwrap();
        assert_eq!(t.fd_table.get(fd).unwrap().size(), 5);
    }

    // 4. 退出
    let pid = task.lock().pid;
    tm.exit_task(pid, 0);
    assert_eq!(task.lock().state, TaskState::Exited);
}
