// =============================================================================
// Feature 9.3: UART 接收（中断模式）— 参考实现
// =============================================================================
//
// 本文件展示如何实现中断驱动的 UART 接收。
// 学生需要在 os/src/driver/uart.rs 中完成以下内容：
//
// 1. RxBuffer — 环形缓冲区（中断→线程的数据通道）
// 2. enable_receive_interrupt() — 配置 IER 使能接收中断
// 3. handle_interrupt() — 中断处理函数，从 RBR 读数据到缓冲区
// 4. getchar() — 优先从缓冲区读取，回退到轮询
// 5. try_getchar() — 非阻塞版本

// =============================================================================
// 第 1 部分：环形缓冲区
// =============================================================================

/// 接收缓冲区容量
pub const RX_BUF_SIZE: usize = 256;

/// 环形缓冲区
///
/// 教学要点：中断处理函数（生产者）写入数据，线程上下文（消费者）
/// 读取数据。使用 count 字段跟踪已存数据量，避免读写指针重叠歧义。
pub struct RxBuffer {
    buf: [u8; RX_BUF_SIZE],
    read_pos: usize,   // 消费者读取位置
    write_pos: usize,  // 生产者写入位置
    count: usize,      // 当前数据量
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

    /// 写入一个字节，缓冲区满时返回 false
    pub fn push(&mut self, byte: u8) -> bool {
        if self.count >= RX_BUF_SIZE {
            return false; // 满时丢弃（背压策略）
        }
        self.buf[self.write_pos] = byte;
        self.write_pos = (self.write_pos + 1) % RX_BUF_SIZE;
        self.count += 1;
        true
    }

    /// 读取一个字节，缓冲区空时返回 None
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

/// 全局接收缓冲区，由 spin::Mutex 保护
pub static RX_BUFFER: spin::Mutex<RxBuffer> = spin::Mutex::new(RxBuffer::new());

// =============================================================================
// 第 2 部分：中断使能方法（在 Uart impl 中）
// =============================================================================

// 使能接收数据就绪中断
//
// 设置 IER 的 ERBFI (bit 0)。使能后，当 RBR 中有数据时，
// UART 会拉高 IRQ 线 → PLIC 聚合 → CPU 触发 supervisor external interrupt。
pub fn enable_receive_interrupt(&self) {
    unsafe {
        self.write_reg(reg::IER, IerFlags::ERBFI);
    }
}

// 禁用所有中断
pub fn disable_interrupts(&self) {
    unsafe {
        self.write_reg(reg::IER, 0x00);
    }
}

// =============================================================================
// 第 3 部分：中断处理函数（在 Uart impl 中）
// =============================================================================

// UART 中断处理
//
// 从 RBR 读取所有可用数据（FIFO 可能有多个字节），存入 RX_BUFFER。
// 此函数在 trap_handler 的外部中断路径中调用。
//
// 调用链：CPU trap → trap_handler(scause=9) → PLIC claim → uart.handle_interrupt()
pub fn handle_interrupt(&self) {
    while self.is_data_ready() {
        let byte = self.read_rbr();
        let mut buf = RX_BUFFER.lock();
        buf.push(byte); // 满时丢弃
    }
}

// =============================================================================
// 第 4 部分：中断驱动的 getchar（在 Uart impl 中）
// =============================================================================

// 阻塞接收：优先从缓冲区读取，回退到轮询
//
// 设计理由：
// - 中断启用时：数据在中断中进入缓冲区，getchar 从缓冲区读取（高效）
// - 中断未启用时：回退到轮询（兼容初始化阶段和无 PLIC 的场景）
pub fn getchar(&self) -> u8 {
    // 1. 先尝试从缓冲区读取
    {
        let mut buf = RX_BUFFER.lock();
        if let Some(byte) = buf.pop() {
            return byte;
        }
    }
    // 2. 缓冲区空 → 回退到轮询
    while !self.is_data_ready() {}
    self.read_rbr()
}

// 非阻塞接收
//
// 仅检查缓冲区，无数据立即返回 None。
// 适用于事件循环中非阻塞检查输入。
pub fn try_getchar(&self) -> Option<u8> {
    let mut buf = RX_BUFFER.lock();
    buf.pop()
}

// =============================================================================
// 第 5 部分：trap_handler 中的外部中断处理
// =============================================================================

// 在 os/src/arch/riscv64/mod.rs 的 trap_handler 中：
//
// scause 9 (Supervisor external interrupt) 的处理需要：
// 1. PLIC claim — 读取中断源编号
// 2. 根据中断源分发到对应驱动（如 UART IRQ=10）
// 3. PLIC complete — 写回中断源编号
//
// 注：PLIC 初始化（9.7）和 claim/complete（9.8）是后续 feature。
// 9.3 只需确保 UART 侧的中断处理代码就绪。
//
// 示例（后续 feature 完善 PLIC 后）：
// ```rust
// 9 => {
//     let claim = plic_claim();
//     match claim {
//         10 => uart.handle_interrupt(),  // UART0 IRQ
//         _ => {}
//     }
//     plic_complete(claim);
// }
// ```
