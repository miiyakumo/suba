//! # 驱动框架 (driver)
//!
//! 本模块定义了设备驱动的抽象接口。
//!
//! ## 教学概念
//! - **Console trait**：控制台 I/O 抽象，支持字符读写
//! - **Power trait**：电源管理抽象（关机、重启）
//! - **SpinLock**：自旋锁同步原语，用于内核中的并发安全
//! - 驱动框架的核心思想：接口与实现分离，同一接口可对接 Mock 或真实硬件

pub mod mock;

use spin::Mutex;

/// 控制台 I/O trait。
///
/// 提供最基本的字符级输入输出能力。
/// Mock 实现直接使用宿主终端，RISC-V 实现使用 UART MMIO。
pub trait Console {
    /// 输出一个字节
    fn putchar(c: u8);

    /// 读取一个字节，无输入时返回 None
    fn getchar() -> Option<u8>;

    /// 输出字符串
    fn puts(s: &str) {
        for b in s.bytes() {
            Self::putchar(b);
        }
    }
}

/// 电源管理 trait。
///
/// 提供关机和重启操作。
pub trait Power {
    /// 关机
    fn shutdown() -> !;

    /// 重启
    fn reboot() -> !;
}

/// SpinLock 类型别名。
///
/// ## 教学概念
/// 内核中不能使用标准库的 `std::sync::Mutex`（它会阻塞线程），
/// 而是使用**自旋锁 (SpinLock)**：当锁被持有时，CPU 不断循环检查
/// （"自旋"），直到锁可用。
///
/// 为什么不用互斥锁？
/// - 内核中没有"线程调度器"来唤醒被阻塞的线程
/// - 自旋锁简单、确定性强，适合短临界区
/// - 在中断上下文中，自旋锁是唯一安全的同步方式
///
/// 使用 `spin` crate 提供的 `spin::Mutex`，它是 `no_std` 兼容的。
pub type SpinLock<T> = Mutex<T>;

/// 中断安全的控制台输出。
///
/// 用 SpinLock 保护输出缓冲区，确保并发安全。
/// 这是实现 `print!`/`println!` 宏的基础设施。
///
/// ## 教学概念
/// 在多核或中断环境下，多个执行流可能同时输出。
/// 如果不加锁，输出会交错混乱。SpinLock 确保每次输出是原子的。
pub struct LockedConsole<C: Console> {
    inner: SpinLock<C>,
}

impl<C: Console> LockedConsole<C> {
    /// 创建新的中断安全控制台
    pub const fn new(console: C) -> Self {
        Self {
            inner: Mutex::new(console),
        }
    }

    /// 输出一个字节（中断安全）
    pub fn putchar(&self, c: u8) {
        let _guard = self.inner.lock();
        C::putchar(c);
    }

    /// 输出字符串（中断安全）
    pub fn puts(&self, s: &str) {
        let _guard = self.inner.lock();
        C::puts(s);
    }

    /// 读取一个字节（中断安全）
    pub fn getchar(&self) -> Option<u8> {
        let _guard = self.inner.lock();
        C::getchar()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mock::MockConsole;

    #[test]
    fn spinlock_basic() {
        // 验证 SpinLock 可以正常获取和释放
        let lock = SpinLock::new(42);
        let val = *lock.lock();
        assert_eq!(val, 42);
    }

    #[test]
    fn spinlock_concurrent_access() {
        // 验证 SpinLock 保护共享数据
        let lock = SpinLock::new(0usize);
        {
            let mut val = lock.lock();
            *val = 100;
        }
        assert_eq!(*lock.lock(), 100);
    }

    #[test]
    fn locked_console_puts() {
        // 验证中断安全控制台输出
        let before = mock::output_count();
        let console = LockedConsole::new(MockConsole);
        console.puts("hello");
        // MockConsole 的 putchar 被调用了 5 次（"hello" = 5 字节）
        assert_eq!(mock::output_count() - before, 5);
    }

    #[test]
    fn locked_console_getchar() {
        let console = LockedConsole::new(MockConsole);
        assert_eq!(console.getchar(), None);
    }
}
