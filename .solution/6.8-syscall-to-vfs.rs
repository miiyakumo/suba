// 连接 sys_write/sys_read 到 VFS 参考实现
//
// 核心改动：在 kernel/src/syscall/impl.rs 中

// === 新增：全局 FdTable ===
use spin::Mutex;
use crate::fs::{FdTable, VfsFile};

static FD_TABLE: Mutex<FdTable> = Mutex::new(FdTable::new());

/// 获取全局 FdTable 的可变引用（用于测试设置）
pub fn with_fd_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut FdTable) -> R,
{
    let mut table = FD_TABLE.lock();
    f(&mut table)
}

// === 修改 sys_write ===
pub fn sys_write(fd: usize, _buf: usize, len: usize) -> usize {
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            // Mock: 使用零字节模拟写入
            // 真实内核中需要 copy_from_user(buf, len)
            let mock_data = alloc::vec![0u8; len];
            match file.write(&mock_data) {
                Ok(n) => n,
                Err(()) => (-1isize) as usize,
            }
        }
        None => (-1isize) as usize,
    }
}

// === 修改 sys_read ===
pub fn sys_read(fd: usize, _buf: usize, len: usize) -> usize {
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            let mut buf = alloc::vec![0u8; len];
            match file.read(&mut buf) {
                Ok(n) => n,  // Mock: 丢弃数据
                Err(()) => (-1isize) as usize,
            }
        }
        None => (-1isize) as usize,
    }
}

// === 测试设置 ===
// 在测试中，先调用 setup_std_fds() 预分配 stdin/stdout/stderr：
fn setup_std_fds() {
    with_fd_table(|table| {
        if table.get(0).is_none() {
            table.open(Box::new(RamFs::new())); // fd 0: stdin
            table.open(Box::new(RamFs::new())); // fd 1: stdout
            table.open(Box::new(RamFs::new())); // fd 2: stderr
        }
    });
}
