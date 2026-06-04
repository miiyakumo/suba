//! # 系统调用实现
//!
//! 实现各个系统调用的具体逻辑。
//!
//! ## 教学概念
//! 每个系统调用函数：
//! 1. 验证用户参数（地址合法性、缓冲区大小等）
//! 2. 执行内核操作
//! 3. 返回结果（成功返回值或错误码）
//!
//! ## VFS 集成
//! sys_write 和 sys_read 通过全局 FdTable 访问 VFS 文件：
//! - 用户传入 fd（文件描述符编号）
//! - 内核通过 FdTable 查找对应的 VfsFile
//! - 调用 VfsFile 的 read/write 方法完成实际 I/O

use spin::Mutex;
use crate::fs::FdTable;

/// 全局文件描述符表。
///
/// 在真实内核中，每个任务有自己的 FdTable（存在 TCB 中）。
/// Phase 1 Mock 环境使用全局表简化测试。
static FD_TABLE: Mutex<FdTable> = Mutex::new(FdTable::new());

/// 获取全局 FdTable 的可变引用。
///
/// 用于测试设置和标准 FD 初始化。
pub fn with_fd_table<F, R>(f: F) -> R
where
    F: FnOnce(&mut FdTable) -> R,
{
    let mut table = FD_TABLE.lock();
    f(&mut table)
}

/// 写入系统调用。
///
/// 将用户缓冲区的数据写入文件描述符。
///
/// # 参数
/// - `fd`: 文件描述符（1 = stdout, 2 = stderr，或其他已打开的 FD）
/// - `buf`: 用户空间缓冲区地址
/// - `len`: 写入字节数
///
/// # 返回值
/// 成功返回写入的字节数，失败返回 -1。
///
/// ## VFS 路径
/// sys_write → FdTable::get_mut(fd) → VfsFile::write(buf)
pub fn sys_write(fd: usize, _buf: usize, len: usize) -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 从当前任务的 TCB 获取 FdTable
    // 2. 通过 FdTable 查找 fd 对应的 VfsFile
    // 3. 从用户空间复制 buf 数据到内核缓冲区
    // 4. 调用 VfsFile::write 写入数据
    // 5. 返回写入的字节数
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            // Mock: 使用 len 个零字节模拟写入
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

/// 读取系统调用。
///
/// 从文件描述符读取数据到用户缓冲区。
///
/// # 参数
/// - `fd`: 文件描述符（0 = stdin，或其他已打开的 FD）
/// - `buf`: 用户空间缓冲区地址
/// - `len`: 读取字节数
///
/// # 返回值
/// 成功返回读取的字节数，失败返回 -1。
///
/// ## VFS 路径
/// sys_read → FdTable::get_mut(fd) → VfsFile::read(buf)
pub fn sys_read(fd: usize, _buf: usize, len: usize) -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 从当前任务的 TCB 获取 FdTable
    // 2. 通过 FdTable 查找 fd 对应的 VfsFile
    // 3. 调用 VfsFile::read 读取数据到内核缓冲区
    // 4. 从内核缓冲区复制到用户空间 buf
    // 5. 返回读取的字节数
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            // 分配临时缓冲区读取数据
            let mut buf = alloc::vec![0u8; len];
            match file.read(&mut buf) {
                Ok(n) => {
                    // Mock: 丢弃读取的数据（真实内核中需要 copy_to_user）
                    // 测试可通过 FdTable 直接验证文件状态
                    n
                }
                Err(()) => -1isize as usize,
            }
        }
        None => -1isize as usize,
    }
}

/// 退出系统调用。
///
/// 终止当前任务。
///
/// # 参数
/// - `exit_code`: 退出码
///
/// # 返回值
/// 不返回（在真实内核中，此函数不会返回）。
pub fn sys_exit(exit_code: i32) -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 获取当前任务的 TCB
    // 2. 将任务状态设为 Exited
    // 3. 记录退出码
    // 4. 触发调度（让出 CPU，不再返回）
    //
    // Mock: 返回退出码（真实内核中不返回）
    exit_code as usize
}

/// 让出 CPU 系统调用。
///
/// 主动让出 CPU 给其他就绪任务。在协作式调度中，
/// 任务必须主动调用 sys_yield 才能让出 CPU。
///
/// # 返回值
/// 始终返回 0（表示让出成功）。
pub fn sys_yield() -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 获取当前任务的 TCB
    // 2. 将任务状态从 Running 改为 Ready
    // 3. 将任务放回调度队列尾部
    // 4. 调用 schedule() 切换到下一个任务
    //
    // Mock 环境下没有全局 current_task，返回 0 即可
    0
}

/// 调整堆大小系统调用。
///
/// 扩展或缩小当前任务的堆空间。sbrk (set break) 是
/// Unix 最经典的内存分配原语——"break" 即堆顶地址。
///
/// # 参数
/// - `increment`: 堆增量（正数扩展，负数缩小，0 查询当前堆顶）
///
/// # 返回值
/// 成功返回旧堆顶地址，失败返回 -1。
pub fn sys_sbrk(increment: isize) -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 获取当前任务的堆顶 (heap_top)
    // 2. 记录旧堆顶，计算新堆顶 = old_brk + increment
    // 3. increment > 0 时：分配物理帧并映射到新虚拟地址
    // 4. increment < 0 时：取消映射并释放物理帧
    // 5. 更新任务的 heap_top，返回旧堆顶
    //
    // Mock 环境下返回固定的堆顶地址
    let _ = increment;
    0x8080_0000
}

/// 获取 PID 系统调用。
///
/// 返回当前任务的进程标识符 (PID)。
/// 这是最简单的信息查询系统调用——不修改任何状态。
///
/// # 返回值
/// 当前任务的 PID。
pub fn sys_getpid() -> usize {
    // TODO: 学生实现 — 在真实内核中需要：
    // 1. 获取当前任务的 TCB
    // 2. 读取 TCB 中的 pid 字段并返回
    //
    // Mock 环境下返回固定 PID
    1
}

#[cfg(test)]
mod tests {
    // 系统调用测试需要完整的任务环境，
    // 将在集成测试中覆盖。
}
