// .solution/9.4-uart-console.rs — UART Console trait 实现参考
//
// 本文件是 feature 9.4 的参考实现。
// 学生应在 os/src/driver/uart.rs 中为 UART 实现 kernel 的 Console trait。
//
// ## 实现要点
//
// 1. 创建 UartConsole 单元结构体
// 2. 实现 suba_kernel::driver::Console trait
// 3. putchar → CONSOLE_UART.putchar() (轮询 MMIO)
// 4. getchar → RX_BUFFER.lock().pop() (缓冲区读取)
// 5. puts → 逐字节输出，处理 \n → \r\n
//
// ## 教学概念：依赖倒置原则
//
// kernel 定义 Console trait（抽象接口），os crate 提供具体实现。
// kernel 不知道 UART 的存在，只通过 Console trait 交互。
//
// 这使得：
// - 测试时可以用 MockConsole 替换
// - 更换硬件时只需修改 os crate，kernel 不变
// - 多个后端可以共存（UART、SBI、VirtIO...）
//
// ## Console trait 定义 (kernel/src/driver/mod.rs)
//
// pub trait Console {
//     fn putchar(c: u8);
//     fn getchar() -> Option<u8>;
//     fn puts(s: &str) { /* 默认实现：逐字节 putchar */ }
// }
//
// 注意：putchar/getchar 是关联函数（无 &self），不使用 self 参数。
// 这意味着实现类型本身不需要实例——直接 UartConsole::putchar() 调用。
// UART 硬件通过模块级静态变量访问。

// 在 os/src/driver/uart.rs 中添加：

use suba_kernel::driver::Console;

/// UART 控制台 — 实现 kernel 的 Console trait
pub struct UartConsole;

/// 全局 UART 实例（UART0 @ 0x1000_0000）
static CONSOLE_UART: Uart = Uart::new(UART0_BASE);

impl Console for UartConsole {
    fn putchar(c: u8) {
        CONSOLE_UART.putchar(c);
    }

    fn getchar() -> Option<u8> {
        let mut buf = RX_BUFFER.lock();
        buf.pop()
    }

    fn puts(s: &str) {
        for b in s.bytes() {
            if b == b'\n' {
                CONSOLE_UART.putchar(b'\r');
            }
            CONSOLE_UART.putchar(b);
        }
    }
}

// 在 os/src/driver/mod.rs 中导出：
// pub type UartConsole = uart::UartConsole;
