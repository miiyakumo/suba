//! NS16550A UART MMIO 寄存器定义
//!
//! 本模块定义了 UART 的寄存器结构和访问方法。
//! 后续 feature 会使用这些定义实现串口输入输出。
//!
// 允许 dead_code：寄存器定义在后续 feature (9.2+) 中使用
#![allow(dead_code)]
//!
//! NS16550A 是最常用的 UART 芯片，QEMU virt 机器使用它作为串口。
//! 通过 MMIO 方式访问其寄存器，实现内核的串口输入输出。
//!
//! ## 教学概念：UART 寄存器布局
//!
//! UART 的 8 个寄存器通过基地址 + 偏移量访问：
//!
//! ```text
//! 偏移  名称  读/写  说明
//! +0    RBR   读     接收缓冲区 (Receive Buffer Register)
//! +0    THR   写     发送保持 (Transmit Holding Register)
//! +1    IER   读/写  中断使能 (Interrupt Enable Register)
//! +2    IIR   读     中断标识 (Interrupt Identification Register)
//! +2    FCR   写     FIFO 控制 (FIFO Control Register)
//! +3    LCR   读/写  线路控制 (Line Control Register)
//! +4    MCR   读/写  调制解调器控制 (Modem Control Register)
//! +5    LSR   读     线路状态 (Line Status Register)
//! +6    MSR   读     调制解调器状态 (Modem Status Register)
//! +7    SCR   读/写  临时寄存器 (Scratch Register)
//! ```
//!
//! 注意：RBR/THR 共享偏移 +0，读时是 RBR，写时是 THR。
//! IIR/FCR 共享偏移 +2，读时是 IIR，写时是 FCR。
//! 这是因为 UART 芯片用读/写方向区分不同的寄存器。
//!
//! ## QEMU virt 机器 UART 地址
//!
//! ```text
//! UART0: 0x1000_0000 (标准串口，连接终端)
//! UART1: 0x1000_0100
//! UART2: 0x1000_0200
//! UART3: 0x1000_0300
//! ```
//!
//! 我们使用 UART0，它是 QEMU 默认的控制台。

use core::ptr::{read_volatile, write_volatile};
use spin::Mutex;

// ============================================================================
// UART 接收缓冲区
// ============================================================================

/// UART 接收环形缓冲区容量
pub const RX_BUF_SIZE: usize = 256;

/// UART 接收环形缓冲区
///
/// ## 教学概念：环形缓冲区 (Ring Buffer)
///
/// 环形缓冲区是内核中常用的数据结构，用于在中断（生产者）和
/// 线程（消费者）之间安全传递数据。
///
/// ```text
///   读指针 (read_pos)     写指针 (write_pos)
///        ↓                      ↓
///   [  A  |  B  |  C  |  _  |  _  ]
/// ```
///
/// - `push()`: 在 write_pos 写入，指针前进
/// - `pop()`:  从 read_pos 读取，指针前进
/// - 缓冲区满时丢弃新数据（背压策略）
/// - 单线程访问不需要额外锁保护（中断上下文中使用）
pub struct RxBuffer {
    buf: [u8; RX_BUF_SIZE],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

impl RxBuffer {
    /// 创建空缓冲区
    pub const fn new() -> Self {
        Self {
            buf: [0; RX_BUF_SIZE],
            read_pos: 0,
            write_pos: 0,
            count: 0,
        }
    }

    /// 写入一个字节
    ///
    /// 缓冲区满时返回 `false`，数据被丢弃。
    pub fn push(&mut self, byte: u8) -> bool {
        if self.count >= RX_BUF_SIZE {
            return false;
        }
        self.buf[self.write_pos] = byte;
        self.write_pos = (self.write_pos + 1) % RX_BUF_SIZE;
        self.count += 1;
        true
    }

    /// 读取一个字节
    ///
    /// 缓冲区空时返回 `None`。
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

/// 全局 UART 接收缓冲区
///
/// 由中断处理函数写入，由 getchar 读取。
/// 使用 spin::Mutex 保护并发访问。
pub static RX_BUFFER: Mutex<RxBuffer> = Mutex::new(RxBuffer::new());

// ============================================================================
// UART MMIO 基地址
// ============================================================================

/// QEMU virt 机器 UART0 MMIO 基地址
pub const UART0_BASE: usize = 0x1000_0000;

// ============================================================================
// UART 寄存器偏移量
// ============================================================================

/// 寄存器偏移量常量
///
/// ## 教学概念：为什么用偏移量而不是绝对地址？
///
/// 同一类型的 UART 芯片可能有多个实例（UART0, UART1, ...），
/// 每个实例的基地址不同，但寄存器偏移量相同。
/// 用偏移量可以让驱动代码复用于不同实例。
pub mod reg {
    /// RBR (Receive Buffer Register) — 接收缓冲区 [只读]
    ///
    /// 读取此寄存器获取接收到的数据。
    /// 读取后硬件自动清除 LSR 的 DR 位。
    pub const RBR: usize = 0;

    /// THR (Transmit Holding Register) — 发送保持寄存器 [只写]
    ///
    /// 写入此寄存器发送数据。
    /// 写入前必须检查 LSR 的 THRE 位确保发送缓冲区为空。
    pub const THR: usize = 0;

    /// IER (Interrupt Enable Register) — 中断使能寄存器 [读/写]
    ///
    /// 控制哪些中断源被使能：
    /// - bit 0: 接收数据就绪中断 (ERBFI)
    /// - bit 1: 发送保持寄存器空中断 (ETBEI)
    /// - bit 2: 接收线路状态中断 (ELSI)
    /// - bit 3: 调制解调器状态中断 (EDSSI)
    pub const IER: usize = 1;

    /// IIR (Interrupt Identification Register) — 中断标识寄存器 [只读]
    ///
    /// 报告当前最高优先级的待处理中断：
    /// - bit 0: 0=有待处理中断, 1=无中断
    /// - bit 3:1: 中断类型编码
    pub const IIR: usize = 2;

    /// FCR (FIFO Control Register) — FIFO 控制寄存器 [只写]
    ///
    /// 控制硬件 FIFO 缓冲区：
    /// - bit 0: 使能 FIFO
    /// - bit 1: 清除接收 FIFO
    /// - bit 2: 清除发送 FIFO
    /// - bit 7:6: 触发级别（FIFO 满到多少触发中断）
    pub const FCR: usize = 2;

    /// LCR (Line Control Register) — 线路控制寄存器 [读/写]
    ///
    /// 配置串口参数：
    /// - bit 1:0: 数据位长度 (5/6/7/8)
    /// - bit 2: 停止位 (1 或 2)
    /// - bit 5:3: 校验位设置
    /// - bit 6: 中断控制
    /// - bit 7: DLAB (除数锁存访问位)
    ///
    /// DLAB=1 时，偏移 +0 和 +1 访问波特率除数寄存器。
    pub const LCR: usize = 3;

    /// MCR (Modem Control Register) — 调制解调器控制寄存器 [读/写]
    ///
    /// - bit 0: DTR (数据终端就绪)
    /// - bit 1: RTS (请求发送)
    /// - bit 3: OUT2 (用于中断控制)
    pub const MCR: usize = 4;

    /// LSR (Line Status Register) — 线路状态寄存器 [只读]
    ///
    /// 报告 UART 的传输状态：
    /// - bit 0 (DR): 数据就绪 (Data Ready) — 接收缓冲区有数据
    /// - bit 5 (THRE): 发送保持寄存器空 (Transmit Holding Register Empty)
    /// - bit 6 (TEMT): 发送器空 (Transmitter Empty)
    pub const LSR: usize = 5;

    /// MSR (Modem Status Register) — 调制解调器状态寄存器 [只读]
    pub const MSR: usize = 6;

    /// SCR (Scratch Register) — 临时寄存器 [读/写]
    ///
    /// 没有硬件功能，可用于临时存储。
    pub const SCR: usize = 7;
}

// ============================================================================
// LSR 状态位
// ============================================================================

/// LSR (Line Status Register) 状态位
///
/// ## 教学概念：轮询 vs 中断
///
/// 串口 I/O 有两种方式：
/// 1. **轮询 (Polling)**: 不断读取 LSR 检查状态（简单但浪费 CPU）
/// 2. **中断 (Interrupt)**: UART 在数据就绪时触发中断（高效但复杂）
///
/// 我们先实现轮询方式（feature 9.2），后续添加中断支持（feature 9.3）。
pub struct LsrFlags;

impl LsrFlags {
    /// DR (Data Ready) — 接收数据就绪
    ///
    /// 此位为 1 时，RBR 中有可读数据。
    /// 读取 RBR 后硬件自动清除此位。
    pub const DR: u8 = 1 << 0;

    /// THRE (Transmit Holding Register Empty)
    ///
    /// 此位为 1 时，THR 为空，可以写入下一个字符。
    /// 写入 THR 后硬件自动清除此位。
    pub const THRE: u8 = 1 << 5;

    /// TEMT (Transmitter Empty)
    ///
    /// 此位为 1 时，发送器完全空（THR 和移位寄存器都空）。
    pub const TEMT: u8 = 1 << 6;
}

// ============================================================================
// IER 中断使能位
// ============================================================================

/// IER (Interrupt Enable Register) 位定义
pub struct IerFlags;

impl IerFlags {
    /// ERBFI: 接收数据就绪中断使能
    pub const ERBFI: u8 = 1 << 0;

    /// ETBEI: 发送保持寄存器空中断使能
    pub const ETBEI: u8 = 1 << 1;

    /// ELSI: 接收线路状态中断使能
    pub const ELSI: u8 = 1 << 2;

    /// EDSSI: 调制解调器状态中断使能
    pub const EDSSI: u8 = 1 << 3;
}

// ============================================================================
// UART MMIO 寄存器访问
// ============================================================================

/// NS16550A UART MMIO 寄存器结构
///
/// 通过基地址 + 偏移量访问各寄存器。
/// 所有读写使用 `volatile` 操作，防止编译器优化。
///
/// ## 教学概念：为什么需要 volatile？
///
/// MMIO 地址不是普通内存——读写有硬件副作用：
/// - 读 LSR 会返回当前状态（不是上次缓存的值）
/// - 写 THR 会触发硬件发送（不能被优化掉）
///
/// 如果不用 `volatile`，编译器可能：
/// - 把多次读合并为一次（错过状态变化）
/// - 把"看似无用"的写操作删掉（硬件收不到命令）
///
/// `read_volatile` / `write_volatile` 强制每次访问都到达硬件。
pub struct Uart {
    /// MMIO 基地址
    base: usize,
}

impl Uart {
    /// 创建 UART 实例
    ///
    /// # 参数
    /// - `base`: MMIO 基地址（如 `UART0_BASE`）
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    /// 读取指定偏移量的寄存器
    ///
    /// # Safety
    ///
    /// 偏移量必须是有效的 UART 寄存器偏移（0-7）。
    #[inline(always)]
    unsafe fn read_reg(&self, offset: usize) -> u8 {
        // SAFETY: 调用者确保偏移量有效，且基地址指向 UART MMIO 区域
        unsafe { read_volatile((self.base + offset) as *const u8) }
    }

    /// 写入指定偏移量的寄存器
    ///
    /// # Safety
    ///
    /// 偏移量必须是有效的 UART 寄存器偏移（0-7）。
    #[inline(always)]
    unsafe fn write_reg(&self, offset: usize, val: u8) {
        // SAFETY: 调用者确保偏移量有效，且基地址指向 UART MMIO 区域
        unsafe { write_volatile((self.base + offset) as *mut u8, val) }
    }

    // --- 便捷方法 ---

    /// 读取 RBR (接收缓冲区)
    pub fn read_rbr(&self) -> u8 {
        // SAFETY: RBR 偏移量 0 是有效的 UART 寄存器
        unsafe { self.read_reg(reg::RBR) }
    }

    /// 写入 THR (发送保持寄存器)
    pub fn write_thr(&self, c: u8) {
        // SAFETY: THR 偏移量 0 是有效的 UART 寄存器
        unsafe { self.write_reg(reg::THR, c) }
    }

    /// 读取 LSR (线路状态)
    pub fn read_lsr(&self) -> u8 {
        // SAFETY: LSR 偏移量 5 是有效的 UART 寄存器
        unsafe { self.read_reg(reg::LSR) }
    }

    /// 写入 IER (中断使能)
    pub fn write_ier(&self, val: u8) {
        // SAFETY: IER 偏移量 1 是有效的 UART 寄存器
        unsafe { self.write_reg(reg::IER, val) }
    }

    /// 读取 IIR (中断标识)
    pub fn read_iir(&self) -> u8 {
        // SAFETY: IIR 偏移量 2 是有效的 UART 寄存器
        unsafe { self.read_reg(reg::IIR) }
    }

    /// 写入 FCR (FIFO 控制)
    pub fn write_fcr(&self, val: u8) {
        // SAFETY: FCR 偏移量 2 是有效的 UART 寄存器
        unsafe { self.write_reg(reg::FCR, val) }
    }

    /// 写入 LCR (线路控制)
    pub fn write_lcr(&self, val: u8) {
        // SAFETY: LCR 偏移量 3 是有效的 UART 寄存器
        unsafe { self.write_reg(reg::LCR, val) }
    }

    /// 写入 MCR (调制解调器控制)
    pub fn write_mcr(&self, val: u8) {
        // SAFETY: MCR 偏移量 4 是有效的 UART 寄存器
        unsafe { self.write_reg(reg::MCR, val) }
    }

    /// 检查 LSR 的 THRE 位（发送缓冲区是否为空）
    pub fn is_thr_empty(&self) -> bool {
        self.read_lsr() & LsrFlags::THRE != 0
    }

    /// 检查 LSR 的 DR 位（是否有接收数据）
    pub fn is_data_ready(&self) -> bool {
        self.read_lsr() & LsrFlags::DR != 0
    }

    // --- 轮询模式 I/O ---

    /// 初始化 UART（轮询模式）
    ///
    /// 设置 8N1（8 数据位，无校验，1 停止位），禁用中断。
    ///
    /// ## 教学概念：UART 初始化
    ///
    /// 使用 UART 前必须配置其工作参数：
    /// 1. 禁用中断（轮询模式不需要中断）
    /// 2. 设置 DLAB 以配置波特率（QEMU 中可省略）
    /// 3. 设置 8N1 数据格式
    /// 4. 使能 FIFO
    /// 5. 设置 MCR
    pub fn init(&mut self) {
        // 禁用中断
        self.write_ier(0x00);

        // 使能 FIFO，清空收发 FIFO
        self.write_fcr(0x07);

        // 设置 8N1: 8 数据位, 无校验, 1 停止位
        // LCR bit 1:0 = 11 (8 数据位)
        self.write_lcr(0x03);

        // 设置 MCR: DTR + RTS
        self.write_mcr(0x03);
    }

    /// 轮询方式发送一个字节
    ///
    /// 等待 THR 为空，然后写入字符。
    ///
    /// ## 教学概念：轮询 (Polling)
    ///
    /// 轮询是最简单的 I/O 方式：不断检查状态寄存器，直到条件满足。
    /// 优点：实现简单，无中断处理复杂性。
    /// 缺点：等待期间 CPU 被占用（busy-waiting）。
    ///
    /// 对于 QEMU 虚拟机，轮询足够高效。
    /// 真实硬件上通常使用中断驱动 I/O。
    pub fn putchar(&self, c: u8) {
        // 等待发送缓冲区为空
        while !self.is_thr_empty() {}
        // 写入字符
        self.write_thr(c);
    }

    /// 轮询方式发送字符串
    pub fn puts(&self, s: &str) {
        for b in s.bytes() {
            // 处理换行符：发送 \r\n
            if b == b'\n' {
                self.putchar(b'\r');
            }
            self.putchar(b);
        }
    }

    /// 接收一个字节（中断驱动，阻塞）
    ///
    /// 优先从软件缓冲区读取（中断模式），缓冲区为空时回退到轮询。
    ///
    /// ## 教学概念：中断驱动 I/O
    ///
    /// 中断模式下，数据到达时 UART 触发中断，中断处理函数将数据
    /// 存入环形缓冲区。`getchar` 从缓冲区读取数据，无需忙等。
    ///
    /// 回退到轮询的原因：中断可能尚未启用（初始化阶段），
    /// 此时直接轮询硬件确保 `getchar` 始终可用。
    pub fn getchar(&self) -> u8 {
        // 优先从接收缓冲区读取（中断模式）
        {
            let mut buf = RX_BUFFER.lock();
            if let Some(byte) = buf.pop() {
                return byte;
            }
        }
        // 回退：轮询模式（中断未启用时）
        while !self.is_data_ready() {}
        self.read_rbr()
    }

    /// 非阻塞接收（中断驱动）
    ///
    /// 从软件缓冲区读取一个字节，无数据时立即返回 `None`。
    ///
    /// ## 教学概念：非阻塞 I/O
    ///
    /// 与阻塞的 `getchar` 不同，`try_getchar` 不会等待。
    /// 适用于事件循环、轮询检查等场景。
    pub fn try_getchar(&self) -> Option<u8> {
        let mut buf = RX_BUFFER.lock();
        buf.pop()
    }

    // --- 中断模式 I/O ---

    /// 使能接收数据就绪中断
    ///
    /// 设置 IER 的 ERBFI (Enable Received Data Available Interrupt) 位。
    /// 当 RBR 中有数据可读时，UART 会触发中断。
    ///
    /// ## 教学概念：UART 中断使能
    ///
    /// IER (Interrupt Enable Register) 控制四种中断源：
    /// - bit 0 (ERBFI): 接收数据就绪 → 数据到达时触发
    /// - bit 1 (ETBEI): 发送缓冲区空 → 可以开始发送下一字节
    /// - bit 2 (ELSI):  接收线路状态变化 → 错误检测
    /// - bit 3 (EDSSI): 调制解调器状态变化
    ///
    /// 中断使能后，UART 会通过 IRQ 线通知 PLIC，
    /// PLIC 再通知 CPU 触发 Supervisor external interrupt。
    pub fn enable_receive_interrupt(&self) {
        // SAFETY: IER 偏移量 1 是有效的 UART 寄存器
        unsafe {
            self.write_reg(reg::IER, IerFlags::ERBFI);
        }
    }

    /// 禁用所有 UART 中断
    pub fn disable_interrupts(&self) {
        // SAFETY: IER 偏移量 1 是有效的 UART 寄存器
        unsafe {
            self.write_reg(reg::IER, 0x00);
        }
    }

    /// UART 中断处理函数
    ///
    /// 从 RBR 读取所有可用数据并存入接收缓冲区。
    /// 应在外部中断处理路径中调用。
    ///
    /// ## 教学概念：中断处理中的缓冲
    ///
    /// 中断处理函数应尽可能快地执行：
    /// 1. 从硬件读取数据（RBR → 缓冲区）
    /// 2. 返回（让出 CPU 给被中断的代码）
    ///
    /// 耗时的数据处理应推迟到线程上下文中进行。
    /// 这就是为什么我们用环形缓冲区在中断和线程之间传递数据。
    pub fn handle_interrupt(&self) {
        // 读取所有可用数据（FIFO 模式下可能有多个字节）
        while self.is_data_ready() {
            let byte = self.read_rbr();
            // 写入全局缓冲区，满时丢弃
            let mut buf = RX_BUFFER.lock();
            buf.push(byte);
        }
    }
}

// ============================================================================
// Console trait 实现
// ============================================================================

/// UART 控制台 — 实现 kernel 的 Console trait
///
/// 将 Console trait 的 putchar/getchar 映射到 UART MMIO 操作。
/// 使用 UART0 静态实例，无需额外状态。
///
/// ## 教学概念：trait 桥接
///
/// kernel 定义了平台无关的 `Console` trait（接口），
/// 每个硬件后端提供具体实现（UART、Mock、SBI 等）。
/// 这就是 **依赖倒置原则**：kernel 依赖抽象接口，不依赖具体实现。
///
/// ```text
/// kernel (trait Console) ← 抽象接口
///     ├── mock::MockConsole    ← 宿主机测试
///     └── uart::UartConsole    ← RISC-V 硬件 (本文件)
/// ```
pub struct UartConsole;

/// 全局 UART 实例
static CONSOLE_UART: Uart = Uart::new(UART0_BASE);

impl suba_kernel::driver::Console for UartConsole {
    /// 输出一个字节（轮询模式）
    fn putchar(c: u8) {
        CONSOLE_UART.putchar(c);
    }

    /// 读取一个字节
    ///
    /// 优先从缓冲区读取（中断模式），无数据时返回 None。
    fn getchar() -> Option<u8> {
        let mut buf = RX_BUFFER.lock();
        buf.pop()
    }

    /// 输出字符串（使用默认实现：逐字节 putchar）
    fn puts(s: &str) {
        for b in s.bytes() {
            if b == b'\n' {
                CONSOLE_UART.putchar(b'\r');
            }
            CONSOLE_UART.putchar(b);
        }
    }
}

// ============================================================================
// 编译期验证
// ============================================================================

/// UART 寄存器偏移量验证
const _: () = assert!(reg::RBR == 0);
const _: () = assert!(reg::THR == 0);
const _: () = assert!(reg::IER == 1);
const _: () = assert!(reg::IIR == 2);
const _: () = assert!(reg::FCR == 2);
const _: () = assert!(reg::LCR == 3);
const _: () = assert!(reg::MCR == 4);
const _: () = assert!(reg::LSR == 5);
const _: () = assert!(reg::MSR == 6);
const _: () = assert!(reg::SCR == 7);

/// LSR 标志位验证
const _: () = assert!(LsrFlags::DR == 0x01);
const _: () = assert!(LsrFlags::THRE == 0x20);
const _: () = assert!(LsrFlags::TEMT == 0x40);

/// IER 标志位验证
const _: () = assert!(IerFlags::ERBFI == 0x01);
const _: () = assert!(IerFlags::ETBEI == 0x02);
const _: () = assert!(IerFlags::ELSI == 0x04);
const _: () = assert!(IerFlags::EDSSI == 0x08);

/// UART0 基地址验证
const _: () = assert!(UART0_BASE == 0x1000_0000);
