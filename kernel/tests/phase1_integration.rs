//! # Phase 1 Mock 完整集成测试
//!
//! 验证 Mock 后端下完整的教学闭环：
//! boot → 创建任务 → 文件 I/O → 任务退出
//!
//! ## 教学概念
//! 这个测试模拟了操作系统启动后的完整流程：
//! 1. 内核初始化（TaskManager）
//! 2. 创建第一个用户任务（自动分配 stdin/stdout/stderr）
//! 3. 任务通过文件描述符进行 I/O
//! 4. 任务退出
//!
//! ## 与真实内核的对应
//! - TaskManager.create_task() ↔ fork/exec
//! - task.fd_table ↔ 进程的文件描述符表
//! - VfsFile::write/read ↔ 用户程序的 write()/read() 系统调用

extern crate alloc;
extern crate suba_kernel;

use alloc::boxed::Box;
use suba_kernel::fs::ramfs::RamFs;
use suba_kernel::task::{TaskManager, TaskState};

/// Phase 1 完整集成测试。
///
/// 模拟：内核启动 → 创建任务 → 文件 I/O → 任务退出。
#[test]
fn phase1_full_cycle() {
    // === 1. 内核启动：初始化 TaskManager ===
    let mut tm = TaskManager::new();
    assert!(tm.is_empty());

    // === 2. 创建第一个用户任务 ===
    let task_handle = tm.create_task(0x8020_0000, 0x8040_0000, 0x8060_0000);
    assert_eq!(tm.len(), 1);

    {
        let task = task_handle.lock();
        assert_eq!(task.pid, 1);
        assert_eq!(task.state, TaskState::Ready);

        // 验证标准文件描述符已预分配
        assert!(task.fd_table.get(0).is_some(), "fd 0 (stdin) 应存在");
        assert!(task.fd_table.get(1).is_some(), "fd 1 (stdout) 应存在");
        assert!(task.fd_table.get(2).is_some(), "fd 2 (stderr) 应存在");
        assert!(task.fd_table.get(3).is_none(), "fd 3 不应存在");
    }

    // === 3. 任务通过文件描述符写入数据 ===
    {
        let mut task = task_handle.lock();

        // 打开一个新文件（fd=3）
        let file = Box::new(RamFs::new());
        let fd = task.fd_table.open(file);
        assert_eq!(fd, 3);

        // 写入数据到文件
        let data = b"hello, suba kernel!";
        let written = task.fd_table.get_mut(fd).unwrap().write(data).unwrap();
        assert_eq!(written, data.len());

        // 写入数据到 stdout
        let stdout_written = task.fd_table.get_mut(1).unwrap().write(b"boot ok").unwrap();
        assert_eq!(stdout_written, 7);
    }

    // === 4. 从文件读取数据验证 ===
    {
        let mut task = task_handle.lock();

        // 验证文件大小
        let file = task.fd_table.get(3).unwrap();
        assert_eq!(file.size(), 19); // "hello, suba kernel!" 长度

        // 读取（从当前 offset，已在末尾，应返回 0）
        let mut buf = [0u8; 32];
        let n = task.fd_table.get_mut(3).unwrap().read(&mut buf).unwrap();
        assert_eq!(n, 0); // 已到末尾
    }

    // === 5. 任务退出 ===
    {
        let pid = task_handle.lock().pid;
        assert!(tm.exit_task(pid, 0));
        assert_eq!(task_handle.lock().state, TaskState::Exited);
        assert_eq!(task_handle.lock().exit_code, 0);
    }

    // === 6. 创建第二个任务验证独立性 ===
    {
        let task2 = tm.create_task(0x8020_0000, 0x8050_0000, 0x8060_0000);
        let t = task2.lock();
        assert_eq!(t.pid, 2);
        assert_eq!(t.state, TaskState::Ready);
        // 第二个任务也有独立的 stdin/stdout/stderr
        assert!(t.fd_table.get(0).is_some());
        assert!(t.fd_table.get(1).is_some());
        assert!(t.fd_table.get(2).is_some());
    }
}
