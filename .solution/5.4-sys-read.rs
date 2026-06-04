//! # sys_read 实现
//!
//! 参考实现：读取系统调用。

/// 读取系统调用。
///
/// 从文件描述符读取数据到用户缓冲区。
///
/// # 参数
/// - `fd`: 文件描述符（0 = stdin）
/// - `buf`: 用户空间缓冲区地址
/// - `len`: 读取字节数
///
/// # 返回值
/// 成功返回读取的字节数，失败返回 -1。
pub fn sys_read(fd: usize, buf: usize, len: usize) -> usize {
    // 只支持 stdin(0)
    if fd != 0 {
        return -1isize as usize;
    }

    if len == 0 {
        return 0;
    }

    // 从控制台读取字符
    let console = crate::driver::Console::lock();
    let mut count = 0;
    let dst = buf as *mut u8;

    for i in 0..len {
        match console.getchar() {
            Some(c) => {
                // SAFETY: dst + i 在用户空间缓冲区范围内
                unsafe { *dst.add(i) = c };
                count += 1;
            }
            None => break,
        }
    }

    count
}
