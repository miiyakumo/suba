/// 用于在早期阶段打印调试信息的控制台模块
///
/// 提供 `eprint!` 和 `eprintln!` 宏，通过 SBI 调用输出到控制台。
/// 这些函数通过宏间接使用，编译器无法追踪调用链。

use core::fmt::{self, Write};

#[allow(dead_code)]
fn console_putchar(c: u8) {
    sbi_rt::console_write_byte(c);
}

#[allow(dead_code)]
pub struct Stdout;

impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.bytes() {
            console_putchar(c);
        }
        Ok(())
    }
}

#[allow(dead_code)]
pub(crate) fn print(args: fmt::Arguments) {
    Stdout.write_fmt(args).unwrap();
}

/// 打印格式化文本到控制台
///
/// 这个宏类似于标准库的 `print!` 宏,但使用 SBI 调用将文本输出到控制台。
/// 它不会在末尾添加换行符。
///
/// # Examples
///
/// ```ignore
/// eprint!("Hello, world!");
/// eprint!("The answer is {}", 42);
/// ```
#[macro_export]
macro_rules! eprint {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::debug_console::print(format_args!($fmt $(, $($arg)+)?))
    }
}

/// 打印格式化文本到控制台并添加换行符
///
/// 这个宏类似于标准库的 `println!` 宏,但使用 SBI 调用将文本输出到控制台。
/// 它会在末尾自动添加换行符。
///
/// # Examples
///
/// ```ignore
/// eprintln!("Hello, world!");
/// eprintln!("The answer is {}", 42);
/// ```
#[macro_export]
macro_rules! eprintln {
    ($fmt: literal $(, $($arg: tt)+)?) => {
        $crate::debug_console::print(format_args!(concat!($fmt, "\n") $(, $($arg)+)?))
    }
}
