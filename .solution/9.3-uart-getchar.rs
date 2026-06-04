// .solution/9.3-uart-getchar.rs — UART 中断驱动接收参考实现
//
// 本文件是 feature 9.3 的参考实现。
// 学生应在 os/src/driver/uart.rs 中实现中断驱动的 UART 接收。
//
// ## 实现要点
//
// 1. RxBuffer 环形缓冲区：中断和线程之间传递数据
// 2. 全局 RX_BUFFER 静态变量
// 3. Uart::enable_receive_interrupt() — 使能 IER ERBFI 位
// 4. Uart::handle_interrupt() — 中断处理：读 RBR → 写入缓冲区
//
// ## 教学概念：中断驱动 I/O
//
// 轮询模式简单但浪费 CPU：线程在等待数据时空转。
// 中断模式让 CPU 去做其他工作，数据到达时由硬件通知。
//
// 数据流：
//   UART 硬件 → 中断 → PLIC → CPU → trap_handler
//     → Uart::handle_interrupt() → RBR → RX_BUFFER
//     → getchar() → 用户程序
//
// ## 环形缓冲区
//
// 中断处理函数（生产者）和读取函数（消费者）通过环形缓冲区解耦。
// 中断上下文不能阻塞，缓冲区满时直接丢弃数据。
//
// ## NS16550A 中断类型
//
// IER (Interrupt Enable Register) 控制四种中断源：
// - bit 0 (ERBFI): 接收数据就绪 — 数据到达 RBR 时触发
// - bit 1 (ETBEI): 发送缓冲区空 — THR 空时触发
// - bit 2 (ELSI):  接收线路状态 — 奇偶校验错误等
// - bit 3 (EDSSI): 调制解调器状态 — CTS/DSR 变化
//
// 我们只需要 ERBFI（接收数据就绪中断）。

use spin::Mutex;

pub const RX_BUF_SIZE: usize = 256;

pub struct RxBuffer {
    buf: [u8; RX_BUF_SIZE],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl RxBuffer {
    pub const fn new() -> Self {
        Self {
            buf: [0; RX_BUF_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }

    pub fn push(&mut self, byte: u8) -> bool {
        if self.count >= RX_BUF_SIZE {
            return false;
        }
        self.buf[self.write_pos] = byte;
        self.write_pos = (self.write_pos + 1) % RX_BUF_SIZE;
        self.count += 1;
        true
    }

    pub fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            return None;
        }
        let byte = self.buf[self.read_pos];
        self.read_pos = (self.read_pos + 1) % RX_BUF_SIZE;
        self.count -= 1;
        Some(byte)
    }
}

pub static RX_BUFFER: Mutex<RxBuffer> = Mutex::new(RxBuffer::new());

// 在 Uart impl 块中添加的方法：
//
// pub fn enable_receive_interrupt(&self) {
//     unsafe { self.write_reg(reg::IER, IerFlags::ERBFI); }
// }
//
// pub fn disable_interrupts(&self) {
//     unsafe { self.write_reg(reg::IER, 0x00); }
// }
//
// pub fn handle_interrupt(&self) {
//     while self.is_data_ready() {
//         let byte = self.read_rbr();
//         let mut buf = RX_BUFFER.lock();
//         buf.push(byte);
//     }
// }
