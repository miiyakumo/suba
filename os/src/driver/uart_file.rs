//! UART 文件抽象 — 实现 VfsFile trait
//!
//! 将 UART 串口包装为 VfsFile，使 sys_write 可以通过
//! 文件描述符表输出到控制台。
//!
//! ## 教学概念：设备即文件 (Everything is a file)
//!
//! Unix 的核心设计哲学之一：设备也用文件接口访问。
//! - `/dev/console` → 控制台输出
//! - `/dev/null` → 空设备
//! - `/dev/zero` → 零设备
//!
//! 我们的 UartFile 就是这个思想的简化版：
//! fd=1 (stdout) → UartFile → UART MMIO → QEMU 终端

use crate::driver::uart::{Uart, UART0_BASE};

/// UART 文件 — 实现 VfsFile trait
///
/// 写入时直接输出到 UART MMIO，读取时从 UART 接收缓冲区读取。
/// 用于 fd=0 (stdin), fd=1 (stdout), fd=2 (stderr)。
pub struct UartFile {
    uart: Uart,
}

impl UartFile {
    /// 创建 UART 文件实例
    pub const fn new() -> Self {
        Self {
            uart: Uart::new(UART0_BASE),
        }
    }
}

impl suba_kernel::fs::VfsFile for UartFile {
    /// 写入数据到 UART
    ///
    /// 将每个字节通过 UART MMIO 发送到 QEMU 终端。
    /// 换行符 `\n` 自动转换为 `\r\n`。
    fn write(&mut self, buf: &[u8]) -> Result<usize, ()> {
        for &b in buf {
            if b == b'\n' {
                self.uart.putchar(b'\r');
            }
            self.uart.putchar(b);
        }
        Ok(buf.len())
    }

    /// 从 UART 读取数据
    ///
    /// 非阻塞读取：从接收缓冲区获取数据，无数据时返回 0。
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut count = 0;
        for slot in buf.iter_mut() {
            match self.uart.try_getchar() {
                Some(byte) => {
                    *slot = byte;
                    count += 1;
                }
                None => break,
            }
        }
        Ok(count)
    }

    /// 文件大小（UART 是流设备，返回 0）
    fn size(&self) -> usize {
        0
    }
}
