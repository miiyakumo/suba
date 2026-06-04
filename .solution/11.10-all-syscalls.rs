// Solution: 11.10 - 系统调用全路径验证
//
// 本文件展示所有系统调用的端到端路径：
// write → 控制台输出, read → 控制台输入, exit → 任务退出,
// yield → 主动让出, getpid → 获取 PID。
//
// 完整代码分布在以下文件中：
//   kernel/src/syscall/impl.rs   — syscall 具体实现
//   kernel/src/syscall/mod.rs    — dispatch 分发
//   os/src/arch/riscv64/mod.rs   — trap_handler, AFTER_SYSCALL 回调
//   os/src/main.rs               — after_syscall_exit, schedule
//   os/src/driver/uart_file.rs   — UartFile (VfsFile → UART)
//   os/src/driver/uart.rs        — UART MMIO 驱动 + RX_BUFFER

// ============================================================================
// 系统调用全路径总览
// ============================================================================
//
// 所有系统调用共享相同的前半段路径：
//
// ```text
// 用户程序: li a7, <syscall_no>; ecall
//   → CPU: sepc←PC, scause←8, sstatus.SPP←User, PC←stvec
//   → trap_entry (trap.S): 保存全部寄存器到 TrapFrame
//   → trap_handler (mod.rs): sepc+=4, dispatch(tf)
//   → kernel::syscall::dispatch: match a7 → 调用对应处理函数
// ```
//
// 区别在后半段——每个系统调用的处理逻辑不同。

// ============================================================================
// 路径 1: sys_write → 控制台输出
// ============================================================================

/// 完整路径:
///   sys_write(fd=1, buf, len)
///     → FD_TABLE.get_mut(1)           // stdout
///     → VfsFile::write(kernel_buf)    // UartFile::write
///       → uart.putchar(byte) * len    // 逐字节通过 UART MMIO 发送
///     → 返回写入字节数
///
/// ## 教学概念：write 的完整链路
///
/// ```text
/// 用户程序                    内核                     硬件
/// ┌──────────┐              ┌──────────────────┐    ┌──────┐
/// │ li a7, 64│  ecall       │ dispatch(tf)      │    │      │
/// │ li a0, 1 │ ──────────►  │   sys_write(1,..) │    │      │
/// │ la a1,buf│              │     FdTable.get(1)│    │      │
/// │ li a2, 5 │              │     UartFile      │    │ UART │
/// │ ecall    │              │       .write()    │───►│ MMIO │
/// │          │ ◄── sret ── │     return 5       │    │      │
/// └──────────┘              └──────────────────┘    └──────┘
/// ```

// ============================================================================
// 路径 2: sys_read → 控制台输入（中断驱动）
// ============================================================================

/// 完整路径:
///   sys_read(fd=0, buf, len)
///     → FD_TABLE.get_mut(0)           // stdin
///     → VfsFile::read(kernel_buf)     // UartFile::read
///       → uart.try_getchar() * len    // 从 RX_BUFFER 读取
///         → RX_BUFFER.lock().pop()    // 非阻塞读取
///     → copy_to_user(kernel_buf, buf) // 复制到用户空间
///     → 返回读取字节数
///
/// ## 教学概念：中断驱动输入路径
///
/// ```text
/// 数据到达（异步）                    读取数据（同步）
/// ┌──────────────────────┐          ┌──────────────────────┐
/// │ 用户按键              │          │ 用户程序: read(0,..) │
/// │   ↓                  │          │   ↓                  │
/// │ UART 中断 → PLIC     │          │ ecall → dispatch     │
/// │   ↓                  │          │   ↓                  │
/// │ trap_handler         │          │ sys_read(0,..)      │
/// │   ↓                  │          │   ↓                  │
/// │ plic::handle_ext     │          │ UartFile::read       │
/// │   ↓                  │          │   ↓                  │
/// │ uart.handle_interrupt│          │ try_getchar()        │
/// │   ↓                  │          │   ↓                  │
/// │ RBR → RX_BUFFER      │          │ RX_BUFFER.pop()      │
/// └──────────────────────┘          └──────────────────────┘
///            │                              ↑
///            └──────────────────────────────┘
///                  通过环形缓冲区传递
/// ```

// ============================================================================
// 路径 3: sys_exit → 任务退出
// ============================================================================

/// 完整路径:
///   sys_exit(exit_code)
///     → 返回 exit_code (kernel crate 的桩实现)
///     → trap_handler 调用 after_syscall_exit(tf)
///     → after_syscall_exit (main.rs):
///       1. 检查 a7 == SYS_EXIT (93)
///       2. 标记当前任务为 Exited
///       3. schedule() 切换到下一个任务
///
/// ## 教学概念：exit 的分层处理
///
/// ```text
/// kernel crate (平台无关)        os crate (硬件后端)
/// ┌──────────────────────┐     ┌──────────────────────────┐
/// │ sys_exit(code)       │     │ after_syscall_exit(tf)   │
/// │   return code (桩)   │ ──► │   mark task Exited       │
/// └──────────────────────┘     │   schedule()             │
///                              │     → context_switch      │
///                              │     → NEXT_TRAP_FRAME     │
///                              │     → sret → 新任务      │
///                              └──────────────────────────┘
/// ```

// ============================================================================
// 路径 4: sys_yield → 主动让出 CPU
// ============================================================================

/// 完整路径:
///   sys_yield()
///     → 返回 0 (kernel crate 的桩实现)
///     → TODO(student): 在真实内核中需要:
///       1. 获取当前任务 TCB
///       2. 状态设为 Ready
///       3. 放回调度队列尾部
///       4. 调用 schedule() 切换到下一个任务
///
/// ## 教学概念：协作式 vs 抢占式调度
///
/// - **抢占式**: 时钟中断强制切换（handle_timer_interrupt 实现）
/// - **协作式**: 任务主动让出（sys_yield 实现）
///
/// 两者互补：时钟中断保证公平性，sys_yield 允许任务在等待 I/O
/// 时主动释放 CPU，提高整体吞吐量。
///
/// TODO(student): 全部系统调用端到端验证
/// 验证 yield 系统调用的完整路径：
/// 1. 用户程序调用 yield()
/// 2. ecall → trap_handler → dispatch → sys_yield()
/// 3. 标记当前任务 Ready, 放回队列
/// 4. schedule() → context_switch → 下一个任务
/// 5. sret 到新任务的用户态

// ============================================================================
// 路径 5: sys_getpid → 获取 PID
// ============================================================================

/// 完整路径:
///   sys_getpid()
///     → 返回 1 (kernel crate 的 Mock 桩实现)
///     → TODO(student): 在真实内核中需要:
///       1. 读取全局 CURRENT_PID (AtomicUsize)
///       2. 返回当前任务的 PID
///
/// ## 教学概念：最简单的系统调用
///
/// getpid 是"纯查询"系统调用——不修改任何状态，不访问硬件：
/// 1. 读取 CURRENT_PID 原子变量
/// 2. 返回
///
/// 这是理解系统调用机制的最佳起点。

// ============================================================================
// dispatch — 系统调用分发
// ============================================================================

/// 系统调用分发函数 (kernel/src/syscall/mod.rs)
///
/// ```text
/// a7 (syscall_id) → match → handler → set_ret
///
///   63 (SYS_READ)   → sys_read(fd, buf, len)
///   64 (SYS_WRITE)  → sys_write(fd, buf, len)
///   93 (SYS_EXIT)   → sys_exit(exit_code)
///   124 (SYS_YIELD) → sys_yield()
///   172 (SYS_GETPID)→ sys_getpid()
///   214 (SYS_SBRK)  → sys_sbrk(increment)
///   其它            → -1 (未知系统调用)
/// ```
pub fn dispatch<F: SyscallFrame>(frame: &mut F) {
    let ret = match frame.syscall_id() {
        number::SYS_WRITE  => impl::sys_write(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_READ   => impl::sys_read(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_EXIT   => impl::sys_exit(frame.arg0() as i32),
        number::SYS_YIELD  => impl::sys_yield(),
        number::SYS_SBRK   => impl::sys_sbrk(frame.arg0() as isize),
        number::SYS_GETPID => impl::sys_getpid(),
        _ => -1isize as usize,
    };
    frame.set_ret(ret);
}

// ============================================================================
// trap_handler — 系统调用后的回调处理
// ============================================================================

/// trap_handler 中的系统调用处理 (os/src/arch/riscv64/mod.rs)
///
/// ```text
/// scause == 8 (U-mode ecall):
///   1. tf.sepc += 4 (跳过 ecall 指令)
///   2. dispatch(tf) (执行系统调用)
///   3. after_syscall(tf) (硬件后端后处理)
///
/// 系统调用后处理回调:
///   - exit: 标记任务 Exited, 调度下一个任务
///   - yield: (TODO) 将任务放回队尾, 调度下一个任务
///   - write/read/getpid: 无需后处理
/// ```

// ============================================================================
// 教学概念总结：系统调用分层架构
// ============================================================================
//
// ```text
// ┌─────────────────────────────────────────────────────────────┐
// │                      用户程序 (U-mode)                       │
// │  write(1, "hello", 5) | read(0, buf, 10) | exit(0)         │
// └──────────────────────────┬──────────────────────────────────┘
//                            │ ecall
// ┌──────────────────────────▼──────────────────────────────────┐
// │                     trap_entry (trap.S)                      │
// │  保存寄存器 → 切换内核栈 → trap_handler                      │
// └──────────────────────────┬──────────────────────────────────┘
//                            │
// ┌──────────────────────────▼──────────────────────────────────┐
// │                   trap_handler (mod.rs)                      │
// │  scause==8 → sepc+=4 → dispatch(tf) → after_syscall(tf)     │
// └──────────────────────────┬──────────────────────────────────┘
//                            │
// ┌──────────────────────────▼──────────────────────────────────┐
// │               kernel::syscall::dispatch (mod.rs)             │
// │  match a7 → sys_xxx() → set_ret(a0=返回值)                  │
// └──────────────────────────┬──────────────────────────────────┘
//                            │
//         ┌──────────────────┼──────────────────┐
//         ▼                  ▼                  ▼
// ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
// │ sys_write    │  │ sys_read     │  │ sys_exit     │
// │ FdTable(1)   │  │ FdTable(0)   │  │ return code  │
// │ UartFile     │  │ UartFile     │  │ (after_syscall│
// │ uart.putchar │  │ uart.try_get │  │  处理退出)    │
// └──────┬───────┘  └──────┬───────┘  └──────┬───────┘
//        │                 │                 │
//        ▼                 ▼                 ▼
// ┌──────────────┐  ┌──────────────┐  ┌──────────────┐
// │ UART MMIO    │  │ RX_BUFFER    │  │ TASK_MANAGER │
// │ (硬件输出)    │  │ (环形缓冲区)  │  │ (标记 Exited) │
// └──────────────┘  └──────────────┘  └──────────────┘
// ```

// ============================================================================
// 系统调用参数传递 (RISC-V ABI)
// ============================================================================
//
// RISC-V LP64 调用约定中的系统调用参数传递：
//
// ```text
// 寄存器  用途          系统调用语义
// ─────────────────────────────────────
// a0 (x10) 参数0/返回值   fd (write/read), exit_code, sbrk increment
// a1 (x11) 参数1          buf (write/read)
// a2 (x12) 参数2          len (write/read)
// a3 (x13) 参数3          (未使用)
// a4 (x14) 参数4          (未使用)
// a5 (x15) 参数5          (未使用)
// a7 (x17) 系统调用号     SYS_WRITE=64, SYS_READ=63, ...
//
// 返回值通过 a0 (x10) 传递：
//   - write/read: 成功时返回字节数
//   - exit: 不返回（任务被终止）
//   - yield: 返回 0
//   - getpid: 返回 PID
//   - 错误: 返回 -1 (usize::MAX)
// ```

// ============================================================================
// 完整的系统调用生命周期
// ============================================================================
//
// 以 sys_write(1, "hello", 5) 为例，追踪从用户态到硬件再回来的完整路径：
//
// Step 1: 用户程序准备参数
//    li a0, 1         # fd = stdout
//    la a1, msg       # buf = 消息地址
//    li a2, 5         # len = 5
//    li a7, 64        # syscall_no = SYS_WRITE
//
// Step 2: 执行 ecall
//    ecall             # 触发环境调用异常
//    # 硬件: sepc←PC(ecall), scause←8, sstatus.SPP←0(U)
//    # 硬件: sstatus.SIE←0, PC←stvec→trap_entry
//
// Step 3: trap_entry (汇编)
//    # 保存 31 个通用寄存器 + sepc, sstatus 到 TrapFrame
//    # 切换到内核栈
//    # call trap_handler
//
// Step 4: trap_handler (mod.rs)
//    scause=8 → sepc+=4 → dispatch(tf)
//
// Step 5: dispatch (mod.rs)
//    match a7: 64(SYS_WRITE) → sys_write(1, buf_ptr, 5)
//
// Step 6: sys_write (impl.rs)
//    FdTable.get(1) → UartFile.write("hello")
//    → uart.putchar('h') → write THR (MMIO)
//    → uart.putchar('e') → write THR (MMIO)
//    ...
//    return 5
//
// Step 7: dispatch 写入返回值
//    frame.set_ret(5) → a0 = 5
//
// Step 8: after_syscall (exit 和 yield 的回调)
//    非 exit/yield → 无需处理
//
// Step 9: trap_handler 返回
//    → trap_entry 检查 NEXT_TRAP_FRAME
//    → null (无调度切换) → 从原 TrapFrame 恢复
//
// Step 10: trap_return → sret
//    # 恢复全部寄存器（a0=5 是返回值）
//    # sret: PC←sepc (ecall 的下一条指令), SIE←1, 特权级←U
//
// Step 11: 用户程序继续执行
//    # a0 = 5 (成功写入 5 字节)
//    # 继续执行 ecall 之后的指令
