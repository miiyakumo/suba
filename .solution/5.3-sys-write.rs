//! # sys_write 实现
//!
//! 参考实现：写入系统调用。

use crate::driver::Console;

/// 写入系统调用。
///
/// 将用户缓冲区的数据写入文件描述符。
///
/// # 参数
/// - `fd`: 文件描述符（1 = stdout, 2 = stderr）
/// - `buf`: 用户空间缓冲区地址
/// - `len`: 写入字节数
///
/// # 返回值
/// 成功返回写入的字节数，失败返回 -1。
pub fn sys_write(fd: usize, buf: usize, len: usize) -> usize {
    // 只支持 stdout(1) 和 stderr(2)
    if fd != 1 && fd != 2 {
        return -1isize as usize;
    }

    if len == 0 {
        return 0;
    }

    // 从用户空间读取数据
    // SAFETY: buf 来自用户空间，len 由用户指定
    // 在真实内核中需要验证地址合法性
    let data = unsafe { core::slice::from_raw_parts(buf as *const u8, len) };

    // 通过控制台输出
    let console = crate::driver::Console::lock();
    for &byte in data {
        console.putchar(byte);
    }

    len
}
