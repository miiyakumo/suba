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
            inner: SpinLock::new(console),
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
