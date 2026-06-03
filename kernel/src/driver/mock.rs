//! # Mock 驱动实现
//!
//! 提供 Console 和 Power trait 的 Mock 实现。
//!
//! ## 教学概念
//! Mock 控制台在 no_std 测试环境中缓冲输出，
//! 让学生在 `cargo test` 时验证内核的输出行为。

use super::{Console, Power};
use core::sync::atomic::{AtomicUsize, Ordering};

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

/// Mock 电源管理实现。
pub struct MockPower;

impl Power for MockPower {
    fn shutdown() -> ! {
        // 在测试中，shutdown 通过 panic 来"终止"
        panic!("MockPower::shutdown called");
    }

    fn reboot() -> ! {
        panic!("MockPower::reboot called");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_console_puts() {
        // 验证 puts 不会 panic
        MockConsole::puts("hello\n");
    }

    #[test]
    fn mock_console_getchar_returns_none() {
        assert_eq!(MockConsole::getchar(), None);
    }
}
