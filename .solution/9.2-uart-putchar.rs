// .solution/9.2-uart-putchar.rs — UART 发送（轮询模式）参考实现
//
// 本文件是 feature 9.2 的参考实现。
// 学生应在 os/src/driver/uart.rs 中实现 putchar/puts 方法。
//
// ## 实现要点
//
// 1. init() — 初始化 UART 为 8N1，使能 FIFO
// 2. putchar() — 轮询 LSR.THRE，写入 THR
// 3. puts() — 遍历字符串调用 putchar，处理 \n → \r\n
// 4. getchar() — 轮询 LSR.DR，读取 RBR
//
// ## 教学概念：轮询 vs 中断
//
// 轮询 (Polling):
//   while !ready() {}  // busy-wait
//   do_io();
//   优点: 实现简单，确定性延迟
//   缺点: 浪费 CPU（busy-waiting）
//
// 中断 (Interrupt):
//   enable_interrupt();
//   do_other_work();   // CPU 可以做其他事
//   // 中断触发时自动调用 handler
//   优点: CPU 不浪费在等待上
//   缺点: 中断处理复杂，延迟不确定
//
// 对于 QEMU 虚拟机，轮询足够高效。
// 真实硬件上通常使用中断驱动 I/O 以提高效率。

// 在 Uart impl 块中添加:

/// 初始化 UART（轮询模式）
pub fn init(&mut self) {
    // 禁用中断
    self.write_ier(0x00);
    // 使能 FIFO，清空收发 FIFO
    self.write_fcr(0x07);
    // 设置 8N1
    self.write_lcr(0x03);
    // 设置 MCR
    self.write_mcr(0x03);
}

/// 轮询方式发送一个字节
pub fn putchar(&self, c: u8) {
    while !self.is_thr_empty() {}
    self.write_thr(c);
}

/// 轮询方式发送字符串
pub fn puts(&self, s: &str) {
    for b in s.bytes() {
        if b == b'\n' {
            self.putchar(b'\r');
        }
        self.putchar(b);
    }
}

/// 轮询方式接收一个字节（阻塞）
pub fn getchar(&self) -> u8 {
    while !self.is_data_ready() {}
    self.read_rbr()
}

// 在 main.rs 中:
// - 创建全局 UART0 实例
// - uart_putchar/uart_puts 委托给 UART0
