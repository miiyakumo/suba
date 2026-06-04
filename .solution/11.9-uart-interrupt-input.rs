// Solution: 11.9 - UART 中断驱动输入
//
// 本文件展示 UART 中断驱动输入的完整实现路径：
// 用户 sys_read → 等待输入 → 用户按键 → UART 中断 → PLIC
// → trap_handler → 唤醒任务 → 返回字符。
//
// 完整代码分布在以下文件中：
//   os/src/driver/uart.rs      — UART 驱动（寄存器、缓冲区、中断处理）
//   os/src/driver/plic.rs      — PLIC 驱动（中断路由、Claim/Complete）
//   os/src/driver/uart_file.rs — UART 的 VfsFile 包装
//   os/src/arch/riscv64/mod.rs — trap_handler 分发
//   os/src/main.rs             — 启动初始化
//   kernel/src/syscall/impl.rs — sys_read 实现

// ============================================================================
// Part 1: UART 中断初始化 — 三层使能 (os/src/driver/uart.rs)
// ============================================================================

/// 初始化 UART 接收中断
///
/// 完整的中断输入链路需要三层初始化：
/// 1. 设备层（UART）：使能 IER, MCR — 设备能产生中断
/// 2. 路由层（PLIC）：设置优先级、使能、阈值 — 中断能到达 CPU
/// 3. CPU 层（sie/sstatus）：使能 SEIE, SIE — CPU 能响应中断
pub fn init_interrupt() {
    // 设置 MCR OUT2 位（PC 兼容 UART 需要此位才能发出中断）
    // MCR = 0x0B = DTR(1) | RTS(1) | OUT2(1)
    unsafe {
        CONSOLE_UART.write_reg(reg::MCR, 0x0B);
    }
    // 使能接收数据就绪中断 (IER bit 0 = ERBFI)
    CONSOLE_UART.enable_receive_interrupt();
}

// ============================================================================
// Part 2: UART 接收缓冲区 — 环形缓冲区 (os/src/driver/uart.rs)
// ============================================================================

/// UART 接收环形缓冲区
///
/// 在中断处理函数（生产者）和 sys_read（消费者）之间传递数据。
///
/// ## 教学概念：环形缓冲区
///
/// ```text
///   读指针 (read_pos)     写指针 (write_pos)
///        ↓                      ↓
///   [  A  |  B  |  C  |  _  |  _  ]
/// ```
///
/// - push(): 中断处理函数写入（生产者）
/// - pop():  sys_read 读取（消费者）
/// - 缓冲区满时丢弃新数据
pub struct RxBuffer {
    buf: [u8; 256],
    read_pos: usize,
    write_pos: usize,
    count: usize,
}

/// 全局 UART 接收缓冲区
///
/// 由中断处理函数写入（生产者），由 getchar/sys_read 读取（消费者）。
/// 使用 spin::Mutex 保护并发访问。
pub static RX_BUFFER: Mutex<RxBuffer> = Mutex::new(RxBuffer::new());

// ============================================================================
// Part 3: UART 中断处理 (os/src/driver/uart.rs)
// ============================================================================

/// UART 中断处理函数
///
/// 从 RBR 读取所有可用数据并存入接收缓冲区。
///
/// ## 教学概念：中断处理的"快进快出"原则
///
/// 中断处理函数应尽可能快地执行：
/// 1. 从硬件读取数据（RBR → 缓冲区）
/// 2. 返回（让出 CPU 给被中断的代码）
///
/// 耗时的数据处理（如字符解析、回显）应推迟到线程上下文中进行。
/// 环形缓冲区就是为此设计的——中断和线程之间的无锁通信信道。
pub fn handle_interrupt(&self) {
    // 读取所有可用数据（FIFO 模式下可能有多个字节）
    while self.is_data_ready() {
        let byte = self.read_rbr();
        let mut buf = RX_BUFFER.lock();
        buf.push(byte);
    }
}

// ============================================================================
// Part 4: PLIC 外部中断处理 (os/src/driver/plic.rs)
// ============================================================================

/// 处理外部中断 — 由 trap_handler 在 scause == 9 时调用
///
/// ## 教学概念：PLIC Claim/Complete 机制
///
/// ```text
/// 1. PLIC 通知 CPU：外部中断 (scause=9)
/// 2. claim(context)：读取 Claim 寄存器 → 获取中断源 ID
///    PLIC 自动清除 Pending 位，防止重复传递
/// 3. 根据 ID 分发到设备驱动
/// 4. complete(context, source)：写入 Complete 寄存器
///    通知 PLIC 处理完毕，允许该中断源再次触发
/// ```
///
/// TODO(student): UART 中断输入路径验证
/// 验证完整的 UART 中断输入链路：
/// 1. 用户按键 → UART RBR 接收数据
/// 2. UART 触发 IRQ → PLIC 记录 Pending
/// 3. PLIC 比较优先级 → 通知 CPU (scause=9)
/// 4. trap_handler 分发到 handle_external_interrupt
/// 5. claim() → UART0_IRQ (10) → uart.handle_interrupt()
/// 6. RBR → RX_BUFFER (中断处理完毕)
/// 7. complete() 通知 PLIC
/// 8. sys_read → UartFile::read → try_getchar → RX_BUFFER.pop()
pub fn handle_external_interrupt() {
    let source = claim(S_MODE_CONTEXT);
    if source == 0 { return; }

    match source as usize {
        UART0_IRQ => {
            let uart = crate::driver::uart::Uart::new(crate::driver::uart::UART0_BASE);
            uart.handle_interrupt();
        }
        _ => {}
    }

    complete(S_MODE_CONTEXT, source);
}

// ============================================================================
// Part 5: trap_handler 分发 (os/src/arch/riscv64/mod.rs)
// ============================================================================

/// trap_handler — 外部中断分发
///
/// scause = (1<<63) | 9 → Supervisor external interrupt
/// → plic::handle_external_interrupt()
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    let scause = read_scause();
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        match scause & !INTERRUPT_BIT {
            9 => {
                // Supervisor external interrupt (SEI)
                // 由 PLIC 触发：外部设备（UART0 等）发出中断
                crate::driver::plic::handle_external_interrupt();
            }
            // ...
        }
    }
}

// ============================================================================
// Part 6: UartFile — VFS 文件包装 (os/src/driver/uart_file.rs)
// ============================================================================

/// UART 文件 — 实现 VfsFile trait
///
/// 将 UART 串口包装为 VfsFile，使 sys_read/sys_write 可以通过
/// 文件描述符表访问控制台。
///
/// ## 教学概念：设备即文件
///
/// Unix 的核心设计哲学：设备也通过文件接口访问。
/// fd=0 (stdin) → UartFile → UART MMIO → QEMU 终端键盘

impl suba_kernel::fs::VfsFile for UartFile {
    /// 从 UART 读取数据（非阻塞）
    ///
    /// 从接收缓冲区获取数据，无数据时返回 0。
    /// 非阻塞设计允许 sys_read 在没有数据时立即返回，
    /// 由上层（如 shell）决定是否重试。
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut count = 0;
        for slot in buf.iter_mut() {
            match self.uart.try_getchar() {
                Some(byte) => { *slot = byte; count += 1; }
                None => break,
            }
        }
        Ok(count)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, ()> { /* ... */ Ok(buf.len()) }
    fn size(&self) -> usize { 0 }
}

// ============================================================================
// Part 7: sys_read 系统调用 (kernel/src/syscall/impl.rs)
// ============================================================================

/// sys_read(fd, buf, len) — 读取文件描述符
///
/// ## 教学概念：sys_read 完整路径
///
/// ```text
/// 用户程序: read(0, buf, 10)
///   → ecall (scause=8, a7=SYS_READ=63)
///   → trap_entry → trap_handler
///   → sepc+=4, dispatch(tf)
///   → sys_read(0, buf, 10)
///     → FD_TABLE.get_mut(0)
///     → VfsFile::read(kernel_buf)
///       → UartFile::read
///         → uart.try_getchar()
///           → RX_BUFFER.lock().pop()
///             → 返回字符（如果有）
///     → copy_to_user(kernel_buf, buf)
///     → 返回读取字节数
///   → sret → 用户程序继续执行
/// ```
///
/// ## 教学概念：中断模式下的 sys_read
///
/// 在中断模式下，sys_read 不等待——它从缓冲区读取已有数据：
/// - 缓冲区有数据 → 立即返回
/// - 缓冲区无数据 → 返回 0（非阻塞）
///
/// 真正的阻塞 I/O 需要在后续实现（如使用等待队列或条件变量）。
/// 当前的非阻塞设计足够用于简单的交互式程序。
pub fn sys_read(fd: usize, _buf: usize, len: usize) -> usize {
    let mut table = FD_TABLE.lock();
    match table.get_mut(fd) {
        Some(file) => {
            let mut kernel_buf = alloc::vec![0u8; len];
            match file.read(&mut kernel_buf) {
                Ok(n) => n,
                Err(()) => (-1isize) as usize,
            }
        }
        None => (-1isize) as usize,
    }
}

// ============================================================================
// Part 8: 启动初始化 (os/src/main.rs)
// ============================================================================

/// rust_main — 启动流程中的 UART 中断初始化
///
/// 在 PLIC 初始化和全局中断使能之间调用：
///
/// ```text
/// Step 6: CLINT init (时钟中断)
/// Step 6: PLIC init (UART0 → S-mode 路由)
/// Step 6: UART interrupt init (IER ERBFI + MCR OUT2)
/// Step 7: 全局中断使能 (sstatus.SIE=1)
/// ```

// ============================================================================
// 教学概念总结：UART 中断输入完整路径
// ============================================================================
//
// ```text
//                         硬件层                   内核层
// ┌─────────────────────────────────────────────────────────────────────┐
// │                                                                     │
// │  用户按键 (键盘)                                                     │
// │    │                                                                │
// │    ▼                                                                │
// │  UART 硬件 (NS16550A)                                               │
// │  数据到达 RBR，触发 IRQ (source 10)                                  │
// │    │                                                                │
// │    ▼                                                                │
// │  PLIC (Platform-Level Interrupt Controller)                         │
// │  记录 Pending bit，比较优先级，通知 CPU                              │
// │    │                                                                │
// │    ▼                                                                │
// │  CPU 触发 Supervisor external interrupt                             │
// │  sepc←PC, scause←(1<<63)|9                                         │
// │  PC←stvec (trap_entry)                                              │
// │    │                                                                │
// │    ▼                                                                │
// │  trap_entry (trap.S)                                                │
// │  保存寄存器 → 调用 trap_handler                                      │
// │    │                                                                │
// │    ▼                                                                │
// │  trap_handler (mod.rs)                                              │
// │  scause=9 → plic::handle_external_interrupt()                       │
// │    │                                                                │
// │    ▼                                                                │
// │  plic::handle_external_interrupt (plic.rs)                          │
// │  1. claim(S_MODE_CONTEXT) → 10 (UART0)                             │
// │  2. uart.handle_interrupt() → RBR → RX_BUFFER                      │
// │  3. complete(S_MODE_CONTEXT, 10)                                    │
// │    │                                                                │
// │    ▼                                                                │
// │  trap.S → trap_return → sret                                        │
// │  返回被中断的代码（用户程序或内核代码）                               │
// │                                                                     │
// │  ... 稍后 ...                                                       │
// │                                                                     │
// │  用户程序调用 read(0, buf, 10)                                      │
// │    │                                                                │
// │    ▼                                                                │
// │  ecall → trap_handler → dispatch → sys_read                        │
// │    │                                                                │
// │    ▼                                                                │
// │  sys_read (impl.rs)                                                 │
// │  FD_TABLE.get(0) → UartFile::read → try_getchar                    │
// │    │                                                                │
// │    ▼                                                                │
// │  try_getchar (uart.rs)                                              │
// │  RX_BUFFER.lock().pop() → 返回字符                                  │
// │    │                                                                │
// │    ▼                                                                │
// │  用户程序收到字符                                                    │
// │                                                                     │
// └─────────────────────────────────────────────────────────────────────┘
// ```
//
// ## 关键设计决策
//
// 1. **为什么用环形缓冲区？**
//    中断是异步的，系统调用是同步的。环形缓冲区是生产者-消费者模式
//    的经典实现，无需复杂的同步原语。
//
// 2. **为什么中断处理中不能 sleep？**
//    中断上下文不是"任务"——它没有自己的栈和上下文。
//    如果在中断中阻塞，整个 CPU 都会停止，包括其他任务。
//
// 3. **三层中断使能的必要性**
//    UART IER（设备层）、PLIC Enable（路由层）、sie SEIE（CPU 层）
//    三层缺一不可。这是嵌入式系统调试中最常见的"中断不工作"原因。
//
// 4. **非阻塞 vs 阻塞 I/O**
//    当前 sys_read 是非阻塞的——缓冲区空时返回 0。
//    真正的阻塞 I/O 需要"等待队列"：任务在数据到达前休眠，
//    中断处理函数在写入数据后唤醒等待的任务。
//    这是后续 feature 的主题。
