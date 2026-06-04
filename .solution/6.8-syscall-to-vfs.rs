//! # 6.8 参考实现：连接 sys_write/sys_read 到 VFS
//!
//! 核心改动：在 kernel/src/syscall/impl.rs 中
//!
//! ## 关键变化
//! 1. 使用 `spin::Lazy` 延迟初始化全局 FdTable（FdTable::new() 非 const fn）
//! 2. sys_write/sys_read 通过 FdTable 查找文件描述符，调用 VfsFile 方法
//! 3. 添加集成测试验证完整路径：syscall → FdTable → VfsFile

use spin::Mutex;
use crate::fs::FdTable;

// === spin::Lazy 延迟初始化 ===
// FdTable::new() 内部使用 Vec，不是 const fn。
// spin::Lazy 在首次 lock 时初始化，解决 static 初始化问题。
static FD_TABLE: spin::Lazy<Mutex<FdTable>> = spin::Lazy::new(|| Mutex::new(FdTable::new()));

/// 获取全局 FdTable 的可变引用（用于测试设置）
pub fn with_fd_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut FdTable) -> R,
{
    let mut table = FD_TABLE.lock();
    f(&mut table)
}

// === sys_write：通过 FdTable → VfsFile ===
pub fn sys_write(fd: usize, _buf: usize, len: usize) -> usize {
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            // Mock: 使用零字节模拟用户数据
            // 真实内核: copy_from_user(buf, kernel_buf, len)
            let mock_data = alloc::vec![0u8; len];
            match file.write(&mock_data) {
                Ok(n) => n,
                Err(()) => (-1isize) as usize,
            }
        }
        None => (-1isize) as usize,
    }
}

// === sys_read：通过 FdTable → VfsFile ===
pub fn sys_read(fd: usize, _buf: usize, len: usize) -> usize {
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            let mut kernel_buf = alloc::vec![0u8; len];
            match file.read(&mut kernel_buf) {
                Ok(n) => {
                    // Mock: 丢弃数据（真实内核: copy_to_user）
                    n
                }
                Err(()) => (-1isize) as usize,
            }
        }
        None => (-1isize) as usize,
    }
}

// === 测试 ===
// 集成测试验证完整路径：
// 1. sys_write 正确写入 VFS（通过 FdTable 验证 size）
// 2. sys_read 正确读取 VFS（验证返回字节数和 EOF）
// 3. 无效 fd 返回 -1
// 4. dispatch → sys_write/read → FdTable → VfsFile 完整链路
