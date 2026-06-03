/// Mock 控制台实现。
///
/// 在测试模式下，`putchar` 统计输出的字符数。
pub struct MockConsole;

/// 输出字符计数器（用于测试验证）
static OUTPUT_COUNT: AtomicUsize = AtomicUsize::new(0);

impl Console for MockConsole {
    fn putchar(_c: u8) {
        OUTPUT_COUNT.fetch_add(1, Ordering::Relaxed);
    }

    fn getchar() -> Option<u8> {
        // Mock: 无输入
        None
    }
}

/// 获取 MockConsole 的输出字符计数（用于测试）
pub fn output_count() -> usize {
    OUTPUT_COUNT.load(Ordering::Relaxed)
}
