//! RISC-V 64 位架构后端
//!
//! 本模块为 suba 内核提供 RISC-V (rv64) 特定的实现：
//! - Context: 上下文切换的寄存器保存/恢复
//! - CpuOps: CSR 寄存器操作（后续 feature）
//! - MmOps: SV39 页表操作（后续 feature）
//!
//! ## 教学概念：RISC-V 特权级
//!
//! RISC-V 有 3 个特权级（M > S > U）：
//! - M-mode (Machine): OpenSBI 运行在此，处理启动和 SBI 调用
//! - S-mode (Supervisor): 内核运行在此，管理页表和中断
//! - U-mode (User): 用户程序运行在此
//!
//! 我们的内核运行在 S-mode，通过 CSR 寄存器与硬件交互。
//! OpenSBI 已在 M-mode 完成硬件初始化，内核从 `_start` 开始执行。

#![allow(unused)] // 后续 feature 会使用这些声明

pub mod page;

use core::arch::asm;
use suba_kernel::arch::{Context, CpuOps, HwTrapFrame, MmOps, PA, SyscallFrame, VA};

// 包含上下文切换汇编代码
core::arch::global_asm!(include_str!("switch.S"));

// 包含陷阱入口/恢复汇编代码
core::arch::global_asm!(include_str!("trap.S"));

// RISC-V 上下文切换的 extern 汇编入口
// 在 switch.S 中实现，保存 callee-saved 寄存器到 old_ctx，
// 从 new_ctx 恢复寄存器并跳转。
//
// Safety: old_ctx 和 new_ctx 必须指向有效的、正确对齐的 Context 结构体。
unsafe extern "C" {
    pub fn switch(old_ctx: *mut Context, new_ctx: *const Context);
}

// 陷阱入口/恢复的 extern 汇编入口
// trap_entry: 由硬件通过 stvec 跳转，保存全部寄存器到 TrapFrame，调用 trap_handler
// trap_return: 从 TrapFrame 恢复全部寄存器，执行 sret 返回
//
// Safety: 调用 trap_return 时 a0 必须指向有效的 TrapFrame
unsafe extern "C" {
    pub fn trap_entry();
    pub fn trap_return(trap_frame: *mut TrapFrame);
}

/// trap_handler — 陷阱分发函数（由 trap.S 调用）
///
/// 当 CPU 发生陷阱时，硬件自动跳转到 trap_entry（汇编），
/// 保存全部寄存器后调用本函数。本函数根据 scause 寄存器的值
/// 将陷阱分发到对应的处理逻辑。
///
/// ## 教学概念：RISC-V 陷阱分发 (Trap Dispatch)
///
/// RISC-V 的陷阱处理是"统一入口，分别处理"：
/// 1. 所有陷阱共享同一个入口地址（stvec → trap_entry）
/// 2. 软件读取 scause 判断陷阱类型
/// 3. 根据类型跳转到不同的处理函数
///
/// 这与 x86 的"中断向量表"不同——x86 硬件直接跳转到不同处理函数。
///
/// ## scause 编码格式
///
/// ```text
/// bit 63: 1=中断(异步), 0=异常(同步)
/// bit 62:0: 具体原因编号
/// ```
///
/// ## 完整的陷阱分发表
///
/// | scause          | 类型 | 含义                    | 处理函数                    |
/// |-----------------|------|-------------------------|-----------------------------|
/// | 0               | 异常 | 指令地址未对齐          | panic                       |
/// | 1               | 异常 | 指令访问错误            | panic                       |
/// | 2               | 异常 | 非法指令                | panic                       |
/// | 3               | 异常 | 断点 (ebreak)           | panic                       |
/// | 5               | 异常 | 加载地址未对齐          | panic                       |
/// | 6               | 异常 | 加载访问错误            | panic                       |
/// | 7               | 异常 | 存储地址未对齐          | panic                       |
/// | 8               | 异常 | U-mode ecall            | sepc+=4, dispatch_syscall   |
/// | 9               | 异常 | S-mode ecall            | sepc+=4, (不应发生)         |
/// | 12              | 异常 | 指令页错误              | panic                       |
/// | 13              | 异常 | 加载页错误              | panic                       |
/// | 15              | 异常 | 存储页错误              | panic                       |
/// | (1<<63)\|1      | 中断 | 软件中断 (SSI)          | (当前忽略)                  |
/// | (1<<63)\|5      | 中断 | 时钟中断 (STI)          | clint::handle_timer         |
/// | (1<<63)\|9      | 中断 | 外部中断 (SEI)          | plic::handle_external       |
///
/// # Safety
/// 必须从 trap.S 中以正确的 TrapFrame 指针调用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    // 读取 scause 寄存器，判断陷阱类型
    // scause 最高位区分中断 (1) vs 异常 (0)
    let scause = read_scause();

    // 最高位 (bit 63) 是中断标志位
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ============================================================
        // 中断处理 (Interrupt) — 异步事件，由硬件触发
        // ============================================================
        // 中断编号 = scause & !INTERRUPT_BIT（去掉最高位）
        match scause & !INTERRUPT_BIT {
            1 => {
                // Supervisor software interrupt (SSI)
                // 由其他 hart 通过写入 CLINT msip 寄存器触发
                // 当前单核实现忽略此中断
            }
            5 => {
                // Supervisor timer interrupt (STI)
                // 由 CLINT 触发：mtime >= mtimecmp
                // 处理：递增 tick 计数、设置下一次中断、检查是否需要调度
                crate::driver::clint::handle_timer_interrupt();
            }
            9 => {
                // Supervisor external interrupt (SEI)
                // 由 PLIC 触发：外部设备（UART0 等）发出中断
                // 处理：claim → 分发到设备驱动 → complete
                crate::driver::plic::handle_external_interrupt();
            }
            _ => {
                panic!("[suba] unexpected interrupt: scause={:#x}", scause);
            }
        }
    } else {
        // ============================================================
        // 异常处理 (Exception) — 同步事件，由当前指令触发
        // ============================================================
        match scause {
            0 => {
                // 指令地址未对齐 (Instruction address misaligned)
                let stval = read_stval();
                panic!(
                    "[suba] instruction misaligned: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            2 => {
                // 非法指令 (Illegal instruction)
                let stval = read_stval();
                panic!(
                    "[suba] illegal instruction: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            5 => {
                // 加载地址未对齐 (Load address misaligned)
                let stval = read_stval();
                panic!(
                    "[suba] load misaligned: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            7 => {
                // 存储地址未对齐 (Store/AMO address misaligned)
                let stval = read_stval();
                panic!(
                    "[suba] store misaligned: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            8 => {
                // Environment call from U-mode（用户态系统调用）
                //
                // ## 教学概念：ecall 指令
                //
                // 用户程序通过 ecall 指令请求内核服务。
                // ecall 会触发异常，CPU 跳转到 stvec（trap_entry）。
                // sepc 指向 ecall 指令本身，需要 +4 跳到下一条指令。
                //
                // 系统调用号在 a7 寄存器，参数在 a0-a5 寄存器。
                // 返回值写入 a0 寄存器。
                tf.sepc = tf.sepc.wrapping_add(4);

                // 调用系统调用分发
                // dispatch 根据 a7 中的系统调用号路由到对应处理函数，
                // 处理完毕后将返回值写入 a0。
                //
                // ## 教学概念：syscall 完整路径
                //
                // ```text
                // 用户程序                    内核
                // ┌──────────┐              ┌──────────────────┐
                // │ li a7, 64│  ecall       │ trap_entry (asm) │
                // │ li a0, 1 │ ──────────►  │   保存寄存器      │
                // │ la a1, buf│             │ trap_handler()   │
                // │ li a2, 5 │             │   scause==8      │
                // │ ecall    │             │   sepc += 4      │
                // │          │             │   dispatch(tf)   │
                // │ (继续执行)│ ◄─ sret ── │   a0 = 返回值    │
                // └──────────┘              └──────────────────┘
                // ```
                suba_kernel::syscall::dispatch(tf);

                // 系统调用后处理回调
                // 用于处理需要硬件后端参与的系统调用（如 exit 触发调度）
                // SAFETY: AFTER_SYSCALL 在 init_exit_handler 中初始化
                unsafe {
                    if let Some(callback) = AFTER_SYSCALL {
                        callback(tf);
                    }
                }
            }
            12 => {
                // 指令页错误 (Instruction page fault)
                // 通常意味着：执行了未映射或无执行权限的地址
                let stval = read_stval();
                panic!(
                    "[suba] instruction page fault: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            13 => {
                // 加载页错误 (Load page fault)
                // 通常意味着：读取了未映射或无读权限的地址
                let stval = read_stval();
                panic!(
                    "[suba] load page fault: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            15 => {
                // 存储页错误 (Store/AMO page fault)
                // 通常意味着：写入了未映射或无写权限的地址
                let stval = read_stval();
                panic!(
                    "[suba] store page fault: sepc={:#x}, stval={:#x}",
                    tf.sepc, stval
                );
            }
            _ => {
                // 未知异常
                let stval = read_stval();
                panic!(
                    "[suba] unexpected exception: scause={:#x}, stval={:#x}, sepc={:#x}",
                    scause, stval, tf.sepc
                );
            }
        }
    }

    // trap_handler 正常返回到 trap_entry。
    //
    // trap_entry 会检查 NEXT_TRAP_FRAME：
    // - 如果非 NULL：切换到了新任务，从新 TrapFrame 恢复并 sret
    // - 如果 NULL：未切换，从原始 TrapFrame 恢复并 sret
    //
    // ## 教学概念：抢占式调度的完整路径
    //
    // ```text
    // 时钟中断 → trap_entry → trap_handler
    //   → handle_timer_interrupt()
    //   → schedule() → context_switch() → NEXT_TRAP_FRAME
    //   → trap_handler 返回
    //   → trap.S 检查 NEXT_TRAP_FRAME → 恢复新任务 → sret
    // ```
}

// ---------------------------------------------------------------------------
// 系统调用后处理回调
// ---------------------------------------------------------------------------

/// 系统调用后处理回调函数指针。
///
/// 在 trap_handler 处理完系统调用后调用，用于处理需要硬件后端参与的操作。
/// 例如：exit 系统调用需要触发任务调度和上下文切换。
///
/// ## 教学概念：回调模式
///
/// 内核核心（kernel crate）不应该直接依赖硬件后端（os crate）。
/// 通过回调模式，硬件后端可以注册自己的处理逻辑，
/// 而不需要修改内核核心代码。
///
/// 这体现了操作系统的分层架构：
/// - 内核核心：提供 syscall dispatch、task 管理等抽象
/// - 硬件后端：实现具体的 trap 处理、上下文切换等
static mut AFTER_SYSCALL: Option<fn(&mut TrapFrame)> = None;

/// 注册系统调用后处理回调。
///
/// 在 rust_main 中初始化任务系统后调用，
/// 注册 exit 等需要调度器参与的系统调用的处理逻辑。
///
/// # Safety
///
/// 必须在中断处理之前调用（单线程初始化阶段）。
pub unsafe fn init_exit_handler(callback: fn(&mut TrapFrame)) {
    // SAFETY: 单线程初始化阶段，中断尚未启用
    unsafe {
        AFTER_SYSCALL = Some(callback);
    }
}

// ---------------------------------------------------------------------------
// 抢占式上下文切换支持
// ---------------------------------------------------------------------------

/// 下一个任务的 TrapFrame 指针（原子变量）。
///
/// 当调度器决定切换任务时，将下一个任务的 TrapFrame 地址
/// 写入此变量。trap_entry 在 trap_handler 返回后检查此变量，
/// 如果非 NULL，则从新 TrapFrame 恢复寄存器并 sret。
///
/// ## 教学概念：抢占式调度的信号机制
///
/// 调度器通过 NEXT_TRAP_FRAME 告诉 trap_entry："请切换到这个任务"。
/// 这是一个无锁信号机制：
/// - 调度器（在中断上下文中）写入新任务的 TrapFrame 地址
/// - trap_entry（在 trap_handler 返回后）读取并执行切换
/// - 使用后清零，防止下次误触发
///
/// 使用 `AtomicPtr` 而非 `Mutex`，因为：
/// 1. 此变量在中断上下文中读写，不能使用可能睡眠的锁
/// 2. 单 CPU 环境下，只需保证编译器不重排序即可
/// 3. `Ordering::Release/Acquire` 保证可见性
#[unsafe(no_mangle)]
pub static NEXT_TRAP_FRAME: core::sync::atomic::AtomicPtr<TrapFrame> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

// ---------------------------------------------------------------------------
// 陷阱初始化
// ---------------------------------------------------------------------------

/// 初始化陷阱处理：设置 stvec CSR 为 trap_entry 地址
///
/// ## 教学概念：stvec CSR
///
/// stvec (Supervisor Trap Vector) 寄存器告诉 CPU：
/// "当发生陷阱时，跳转到这个地址"。
///
/// stvec 的格式：
/// - bits 63:2: 基地址（必须 4 字节对齐）
/// - bits 1:0: 陷阱模式
///   - 0 (Direct): 所有陷阱跳转到基地址
///   - 1 (Vectored): 中断跳转到 base+4*cause，异常跳转到 base
///
/// 我们使用 Direct 模式——所有陷阱统一进入 trap_entry，
/// 由软件根据 scause 分发。这比 Vectored 模式更灵活。
pub fn init_trap() {
    // SAFETY: 写入 stvec 是初始化阶段的安全操作
    unsafe {
        asm!(
            "csrw stvec, {addr}",
            addr = in(reg) trap_entry as usize,
            options(nomem, nostack)
        );
    }
}

// ---------------------------------------------------------------------------
// S/U Mode 切换
// ---------------------------------------------------------------------------

/// 从 S-mode 切换到 U-mode 并跳转到用户入口
///
/// 本函数设置一个用户态陷阱帧，然后通过 `trap_return` (即 `sret`) 切换到 U-mode。
///
/// ## 教学概念：RISC-V 特权级切换
///
/// RISC-V 的特权级切换通过 `sret` 指令完成：
/// 1. `sret` 将 `sstatus.SPP` 的值作为目标特权级（0=User, 1=Supervisor）
/// 2. `sret` 将 `sepc` 的值写回 PC（跳转到用户入口）
/// 3. `sret` 将 `sstatus.SPIE` 的值恢复到 `sstatus.SIE`（启用中断）
///
/// 因此，切换到 U-mode 需要：
/// - sstatus.SPP = 0 (User)
/// - sstatus.SPIE = 1 (sret 后启用中断)
/// - sstatus.SIE = 0 (当前关闭中断，sret 后由 SPIE 恢复)
/// - sepc = 用户入口地址
/// - x2_sp = 用户栈指针
/// - kernel_sp = 内核栈指针（从 U-mode 陷阱回来时使用）
///
/// ## 切换流程
///
/// ```text
/// S-mode (内核)
///   ├── 设置 TrapFrame (SPP=User, sepc=user_entry)
///   └── trap_return → sret
///       └── U-mode (用户程序从 sepc 开始执行)
/// ```
///
/// # Safety
///
/// - `user_entry` 必须是有效的用户态代码地址
/// - `user_sp` 必须是有效的用户栈顶地址
/// - `kernel_sp` 必须是有效的内核栈顶地址
/// - `argc` 传递给用户程序的参数（写入 a0 寄存器）
pub unsafe fn drop_to_user_mode(user_entry: usize, user_sp: usize, kernel_sp: usize, argc: usize) {
    // 创建用户态陷阱帧
    let mut tf = TrapFrame::new();

    // 设置 sepc = 用户入口地址
    // sret 会将 sepc 写回 PC
    tf.sepc = user_entry;

    // 设置 sstatus
    // SPP (bit 8) = 0: sret 返回到 U-mode
    // SPIE (bit 5) = 1: sret 后恢复中断使能
    // SIE (bit 1) = 0: 当前中断关闭
    tf.sstatus = 1 << 5; // SPIE=1, SPP=0, SIE=0

    // 设置用户栈指针
    tf.x2_sp = user_sp;

    // 设置内核栈指针（从 U-mode 陷入时 trap_entry 会用到）
    tf.kernel_sp = kernel_sp;

    // a0 = argc：用户程序通过 a0 获取参数个数
    tf.x10_a0 = argc;

    // 通过 trap_return 切换到用户态
    // trap_return 会恢复所有寄存器并执行 sret
    // SAFETY: tf 是刚刚创建的有效 TrapFrame
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }
}

/// 配置用户态陷阱帧（不立即切换）
///
/// 与 `drop_to_user_mode` 不同，本函数仅配置陷阱帧，不执行 `sret`。
/// 适用于需要在切换前做更多准备工作的场景（如设置用户空间页表）。
///
/// # Safety
///
/// 调用者必须确保传入的地址和栈指针有效。
pub unsafe fn setup_user_trap_frame(
    tf: &mut TrapFrame,
    user_entry: usize,
    user_sp: usize,
    kernel_sp: usize,
    argc: usize,
) {
    tf.sepc = user_entry;
    // SPP=0 (User), SPIE=1, SIE=0
    tf.sstatus = 1 << 5;
    tf.x2_sp = user_sp;
    tf.kernel_sp = kernel_sp;
    // a0 = argc：用户程序通过 a0 获取参数个数
    tf.x10_a0 = argc;
}

// ---------------------------------------------------------------------------
// CSR 读取辅助函数
// ---------------------------------------------------------------------------

/// 读取 scause 寄存器（陷阱原因）
///
/// scause 编码了陷阱的类型和具体原因：
/// - bit 63 = 0: 异常（同步，由指令触发）
/// - bit 63 = 1: 中断（异步，由硬件触发）
/// - bit 62:0 = 具体原因编号
#[inline]
fn read_scause() -> usize {
    let scause: usize;
    // SAFETY: scause 是只读 CSR
    unsafe {
        asm!("csrr {}, scause", out(reg) scause, options(nomem, nostack));
    }
    scause
}

/// 读取 stval 寄存器（陷阱附加信息）
///
/// stval 提供陷阱的附加信息，如：
/// - 缺页异常: 导致异常的虚拟地址
/// - 非法指令: 指令编码
/// - 对齐错误: 导致错误的地址
#[inline]
fn read_stval() -> usize {
    let stval: usize;
    // SAFETY: stval 是只读 CSR
    unsafe {
        asm!("csrr {}, stval", out(reg) stval, options(nomem, nostack));
    }
    stval
}

/// 读取 sepc 寄存器（异常程序计数器）
///
/// sepc 保存陷阱发生时的指令地址。
/// sret 指令会将 sepc 写回 PC。
#[inline]
fn read_sepc() -> usize {
    let sepc: usize;
    // SAFETY: sepc 是只读 CSR
    unsafe {
        asm!("csrr {}, sepc", out(reg) sepc, options(nomem, nostack));
    }
    sepc
}


// ---------------------------------------------------------------------------
// 常量
// ---------------------------------------------------------------------------

/// sstatus 寄存器中 SIE (Supervisor Interrupt Enable) 位的掩码
///
/// sstatus 是 RISC-V 的 Supervisor 状态寄存器。
/// SIE 位（bit 1）控制 S-mode 中断是否启用：
/// - SIE=1: S-mode 中断启用
/// - SIE=0: S-mode 中断禁用
const SSTATUS_SIE: usize = 1 << 1;

// ---------------------------------------------------------------------------
// Riscv64CpuOps — RISC-V CPU 操作实现
// ---------------------------------------------------------------------------

/// RISC-V 64 位 CPU 操作实现。
///
/// 通过 CSR (Control and Status Register) 指令与硬件交互。
///
/// ## 教学概念：CSR 操作
///
/// RISC-V 的 CSR 操作是原子的：
/// - `csrr rd, csr` — 读取 CSR 到寄存器
/// - `csrw csr, rs` — 写入寄存器到 CSR
/// - `csrs csr, rs` — 原子设置位（OR）
/// - `csrc csr, rs` — 原子清除位（AND NOT）
/// - `csrrc rd, csr, rs` — 原子清除位并返回旧值
///
/// 我们使用 `csrrc` 实现 disable_interrupts 的原子性。
pub struct Riscv64CpuOps;

impl CpuOps for Riscv64CpuOps {
    /// 获取当前 CPU 核心 ID
    ///
    /// 读取 `mhartid` CSR。在 QEMU virt 机器上，
    /// OpenSBI 启动时为每个 hart 分配唯一 ID。
    #[inline]
    fn id() -> usize {
        let id: usize;
        // SAFETY: mhartid 是只读 CSR，随时可读
        unsafe {
            asm!(
                "csrr {}, mhartid",
                out(reg) id,
                options(nomem, nostack)
            );
        }
        id
    }

    /// 停止 CPU，等待中断唤醒
    ///
    /// WFI (Wait For Interrupt) 指令让 CPU 进入低功耗等待状态，
    /// 直到有中断到来。这是 idle 循环的核心指令。
    fn halt() -> ! {
        loop {
            // SAFETY: wfi 是 S-mode 合法指令
            unsafe { asm!("wfi", options(nomem, nostack)) };
        }
    }

    /// 原子地禁用中断并返回之前的中断状态
    ///
    /// 使用 `csrrc` 指令原子地清除 sstatus.SIE 位。
    /// 返回值包含旧的 SIE 状态，用于后续恢复。
    ///
    /// ## 教学概念
    /// `csrrc rd, csr, rs` 是 "atomic read and clear bits"：
    /// 1. 读取 csr 的旧值到 rd
    /// 2. 将 csr 中 rs 为 1 的位清零
    /// 这两步是原子的，不会被中断打断。
    #[inline]
    fn disable_interrupts() -> usize {
        let old: usize;
        // SAFETY: csrrc 是原子操作，清除 sstatus.SIE 位
        unsafe {
            asm!(
                "csrrc {old}, sstatus, {mask}",
                old = out(reg) old,
                mask = in(reg) SSTATUS_SIE,
                options(nomem, nostack)
            );
        }
        old
    }

    /// 恢复之前保存的中断状态
    ///
    /// 如果 `flags` 中 SIE 位为 1，则重新启用中断。
    ///
    /// # Safety
    ///
    /// `flags` 必须来自 `disable_interrupts()` 的返回值。
    #[inline]
    unsafe fn restore_interrupt_state(flags: usize) {
        if flags & SSTATUS_SIE != 0 {
            Self::enable_interrupts();
        }
    }

    /// 显式启用 S-mode 中断
    ///
    /// 使用 `csrs` 指令原子地设置 sstatus.SIE 位。
    #[inline]
    fn enable_interrupts() {
        // SAFETY: csrs 是原子操作，设置 sstatus.SIE 位
        unsafe {
            asm!(
                "csrs sstatus, {mask}",
                mask = in(reg) SSTATUS_SIE,
                options(nomem, nostack)
            );
        }
    }

    /// 查询当前中断是否启用
    ///
    /// 读取 sstatus.SIE 位。
    #[inline]
    fn interrupts_enabled() -> bool {
        let sstatus: usize;
        // SAFETY: csrr 是只读操作
        unsafe {
            asm!(
                "csrr {}, sstatus",
                out(reg) sstatus,
                options(nomem, nostack)
            );
        }
        (sstatus & SSTATUS_SIE) != 0
    }
}

// ============================================================================
// Riscv64MmOps — RISC-V 内存管理操作
// ============================================================================

/// RISC-V 64 位内存管理操作实现。
///
/// 通过 SV39 页表和 CSR 寄存器实现地址翻译、TLB 管理等。
///
/// ## 教学概念：MmOps trait
///
/// MmOps 是内核与 MMU 硬件之间的桥梁：
/// - `translate_va`: 使用页表将虚拟地址翻译为物理地址
/// - `flush_tlb`: 刷新 TLB 缓存（修改页表后必须刷新）
/// - `current_page_table`: 读取当前页表根地址（satp.PPN）
/// - `switch_page_table`: 切换到新的页表（写入 satp）
pub struct Riscv64MmOps;

impl MmOps for Riscv64MmOps {
    /// 将虚拟地址翻译为物理地址
    ///
    /// 使用当前页表（satp.PPN）进行三级页表遍历。
    ///
    /// ## 教学概念：地址翻译流程
    ///
    /// 1. 读取 satp 获取根页表 PPN
    /// 2. 从 VPN[2] → VPN[1] → VPN[0] 逐级查找
    /// 3. 找到叶子 PTE，计算物理地址 = PPN × 4096 + offset
    ///
    /// # Safety
    ///
    /// 调用者必须确保虚拟地址在当前页表中有有效映射。
    unsafe fn translate_va(va: VA) -> Option<PA> {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF; // 低 44 位是 PPN
        match page::translate_va(va.as_usize(), root_ppn, page::phys_read_u64) {
            Ok(pa) => Some(PA::new(pa)),
            Err(_) => None,
        }
    }

    /// 刷新全部 TLB 条目
    ///
    /// 执行 `sfence.vma` 指令，清除所有 TLB 缓存。
    /// 在修改页表后必须调用，否则旧的翻译结果可能被使用。
    fn flush_tlb() {
        flush_tlb_all();
    }

    /// 刷新指定虚拟地址的 TLB 条目
    ///
    /// 执行 `sfence.vma vaddr, zero`，仅清除特定地址的 TLB 缓存。
    /// 比全部刷新更高效，适用于单页映射变更。
    fn flush_tlb_addr(addr: usize) {
        flush_tlb_addr(addr);
    }

    /// 获取当前页表根物理地址
    ///
    /// 读取 satp 寄存器，提取 PPN 字段并转换为物理地址。
    ///
    /// satp 格式：MODE[63:60] | ASID[59:44] | PPN[43:0]
    fn current_page_table() -> PA {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF; // 低 44 位是 PPN
        PA::new(root_ppn << 12) // PPN → 物理地址
    }

    /// 切换到新的页表
    ///
    /// 写入 satp 寄存器：MODE=SV39(8) | PPN，并执行 sfence.vma。
    ///
    /// # Safety
    ///
    /// `pt_root` 必须指向有效的 SV39 页表。
    unsafe fn switch_page_table(pt_root: PA) {
        let root_ppn = pt_root.as_usize() >> 12;
        // SAFETY: 调用者确保 pt_root 指向有效页表
        unsafe {
            switch_page_table(root_ppn);
        }
    }
}

// ============================================================================
// TrapFrame — RISC-V 陷阱帧
// ============================================================================

/// RISC-V 64 位陷阱帧。
///
/// 保存陷阱（中断/异常/系统调用）发生时的全部寄存器状态。
/// 由 trap_entry.S 在进入内核时保存，trap_handler 处理完毕后恢复。
///
/// ## 教学概念：陷阱帧 vs 上下文
///
/// - **Context** (switch.S): 仅保存 callee-saved 寄存器（14 个），用于自愿切换
/// - **TrapFrame** (trap_entry.S): 保存全部 31 个通用寄存器 + CSR，用于中断/异常
///
/// 陷阱帧必须保存所有寄存器，因为中断可能发生在任何位置，
/// 编译器无法预测哪些寄存器正在被使用。
///
/// ## 结构体布局
///
/// `#[repr(C)]` 确保字段按声明顺序排列，与汇编代码中的偏移量一致。
/// trap_entry.S 通过 sp + offset 访问每个字段。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct TrapFrame {
    /// sepc — Supervisor Exception Program Counter
    /// 陷阱发生时的指令地址，返回时写回 PC
    pub sepc: usize,        // offset 0

    /// x1 (ra) — 返回地址
    pub x1_ra: usize,       // offset 8
    /// x2 (sp) — 栈指针
    pub x2_sp: usize,       // offset 16
    /// x3 (gp) — 全局指针
    pub x3_gp: usize,       // offset 24
    /// x4 (tp) — 线程指针
    pub x4_tp: usize,       // offset 32
    /// x5 (t0) — 临时寄存器 0
    pub x5_t0: usize,       // offset 40
    /// x6 (t1) — 临时寄存器 1
    pub x6_t1: usize,       // offset 48
    /// x7 (t2) — 临时寄存器 2
    pub x7_t2: usize,       // offset 56
    /// x8 (s0/fp) — callee-saved / 帧指针
    pub x8_s0: usize,       // offset 64
    /// x9 (s1) — callee-saved
    pub x9_s1: usize,       // offset 72
    /// x10 (a0) — 参数 0 / 返回值
    pub x10_a0: usize,      // offset 80
    /// x11 (a1) — 参数 1
    pub x11_a1: usize,      // offset 88
    /// x12 (a2) — 参数 2
    pub x12_a2: usize,      // offset 96
    /// x13 (a3) — 参数 3
    pub x13_a3: usize,      // offset 104
    /// x14 (a4) — 参数 4
    pub x14_a4: usize,      // offset 112
    /// x15 (a5) — 参数 5
    pub x15_a5: usize,      // offset 120
    /// x16 (a6) — 系统调用扩展号
    pub x16_a6: usize,      // offset 128
    /// x17 (a7) — 系统调用号
    pub x17_a7: usize,      // offset 136
    /// x18 (s2) — callee-saved
    pub x18_s2: usize,      // offset 144
    /// x19 (s3) — callee-saved
    pub x19_s3: usize,      // offset 152
    /// x20 (s4) — callee-saved
    pub x20_s4: usize,      // offset 160
    /// x21 (s5) — callee-saved
    pub x21_s5: usize,      // offset 168
    /// x22 (s6) — callee-saved
    pub x22_s6: usize,      // offset 176
    /// x23 (s7) — callee-saved
    pub x23_s7: usize,      // offset 184
    /// x24 (s8) — callee-saved
    pub x24_s8: usize,      // offset 192
    /// x25 (s9) — callee-saved
    pub x25_s9: usize,      // offset 200
    /// x26 (s10) — callee-saved
    pub x26_s10: usize,     // offset 208
    /// x27 (s11) — callee-saved
    pub x27_s11: usize,     // offset 216
    /// x28 (t3) — 临时寄存器 3
    pub x28_t3: usize,      // offset 224
    /// x29 (t4) — 临时寄存器 4
    pub x29_t4: usize,      // offset 232
    /// x30 (t5) — 临时寄存器 5
    pub x30_t5: usize,      // offset 240
    /// x31 (t6) — 临时寄存器 6
    pub x31_t6: usize,      // offset 248

    /// sstatus — Supervisor Status Register
    /// 保存陷阱发生时的中断使能、特权级等状态
    pub sstatus: usize,     // offset 256

    /// 内核栈指针
    /// 在 trap_entry 中用于切换到内核栈
    pub kernel_sp: usize,   // offset 264
}

// 编译期检查：TrapFrame 大小 = 34 × 8 = 272 字节
const _: () = assert!(core::mem::size_of::<TrapFrame>() == 272);

// 编译期检查：TrapFrame 字段偏移量与 trap.S 汇编代码一致
//
// ## 教学概念：编译期偏移量验证
//
// trap.S 通过硬编码偏移量（如 `sd ra, 8(a0)`）访问 TrapFrame 字段。
// 如果 Rust 结构体的字段顺序或大小与汇编代码不匹配，会导致数据错位——
// 这种 bug 极难调试。
//
// 我们使用 `core::mem::offset_of!` 宏在编译期验证每个字段的偏移量。
// 如果任何断言失败，编译会报错，而不是在运行时出现神秘的 crash。
//
// 这是 OS 开发中的最佳实践：将"信任边界"从运行时移到编译期。
const _: () = assert!(core::mem::offset_of!(TrapFrame, sepc) == 0);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x1_ra) == 8);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x2_sp) == 16);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x3_gp) == 24);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x4_tp) == 32);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x5_t0) == 40);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x6_t1) == 48);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x7_t2) == 56);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x8_s0) == 64);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x9_s1) == 72);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x10_a0) == 80);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x11_a1) == 88);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x12_a2) == 96);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x13_a3) == 104);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x14_a4) == 112);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x15_a5) == 120);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x16_a6) == 128);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x17_a7) == 136);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x18_s2) == 144);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x19_s3) == 152);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x20_s4) == 160);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x21_s5) == 168);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x22_s6) == 176);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x23_s7) == 184);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x24_s8) == 192);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x25_s9) == 200);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x26_s10) == 208);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x27_s11) == 216);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x28_t3) == 224);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x29_t4) == 232);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x30_t5) == 240);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x31_t6) == 248);
const _: () = assert!(core::mem::offset_of!(TrapFrame, sstatus) == 256);
const _: () = assert!(core::mem::offset_of!(TrapFrame, kernel_sp) == 264);

impl TrapFrame {
    /// 创建全零初始化的陷阱帧
    pub const fn new() -> Self {
        // SAFETY: 所有字段初始化为零是安全的
        unsafe { core::mem::zeroed() }
    }
}

// --- HwTrapFrame trait 实现 ---

impl HwTrapFrame for TrapFrame {
    fn zero_init() -> Self {
        Self::new()
    }

    /// 设置内核线程的初始陷阱帧
    ///
    /// 用于创建新的内核线程时设置初始寄存器状态：
    /// - sepc = 入口地址（SRET 后从这里开始执行）
    /// - sstatus.SPP = Supervisor（返回到 S-mode）
    /// - sstatus.SPIE = 1（SRET 后启用中断）
    /// - sstatus.SIE = 0（当前关闭中断）
    /// - x1_ra = terminal（线程结束时的清理函数）
    /// - x2_sp = kernel_sp（内核栈顶）
    fn set_kernel_trap_frame(&mut self, entry: usize, terminal: usize, kernel_sp: usize) {
        // sstatus: SPP=Supervisor, SPIE=1, SIE=0
        // SPP (bit 8) = 1: 表示从 S-mode 陷阱返回
        // SPIE (bit 5) = 1: SRET 后恢复为 SIE
        // SIE (bit 1) = 0: 当前中断关闭
        let sstatus: usize = (1 << 8) | (1 << 5);
        self.sepc = entry;
        self.sstatus = sstatus;
        self.kernel_sp = kernel_sp;
        self.x1_ra = terminal;
        self.x2_sp = kernel_sp;
    }

    fn get_sp(&self) -> usize {
        self.x2_sp
    }

    fn set_sp(&mut self, val: usize) {
        self.x2_sp = val;
    }

    fn set_a0(&mut self, val: usize) {
        self.x10_a0 = val;
    }

    fn set_a1(&mut self, val: usize) {
        self.x11_a1 = val;
    }

    fn set_a2(&mut self, val: usize) {
        self.x12_a2 = val;
    }

    fn set_ra(&mut self, val: usize) {
        self.x1_ra = val;
    }

    fn set_sepc(&mut self, pc: usize) {
        self.sepc = pc;
    }

    fn get_sepc(&self) -> usize {
        self.sepc
    }
}

// ============================================================================
// 用户态陷阱帧设置 — S/U Mode 切换
// ============================================================================

impl TrapFrame {
    /// 设置用户态的初始陷阱帧
    ///
    /// 用于从 S-mode 切换到 U-mode：设置 sstatus.SPP = User，
    /// sepc = 用户入口地址，SPIE = 1（sret 后启用中断）。
    ///
    /// ## 教学概念：S/U Mode 切换
    ///
    /// RISC-V 的特权级切换通过 `sret` 指令完成：
    /// 1. `sstatus.SPP` 决定 sret 后的特权级：
    ///    - SPP = 0 (User): sret 后进入 U-mode
    ///    - SPP = 1 (Supervisor): sret 后留在 S-mode
    /// 2. `sepc` 决定 sret 后的 PC（从哪里开始执行）
    /// 3. `sstatus.SPIE` 决定 sret 后的中断状态：
    ///    - sret 自动将 SPIE 恢复到 SIE 位
    ///    - 所以 SPIE=1 意味着 sret 后中断启用
    ///
    /// ## sstatus 位域
    ///
    /// ```text
    /// bit 8 (SPP):  0 = User, 1 = Supervisor
    /// bit 5 (SPIE): 1 = sret 后启用中断
    /// bit 1 (SIE):  0 = 当前中断关闭
    /// ```
    ///
    /// # 参数
    /// - `entry`: 用户程序入口地址（sret 后的 PC）
    /// - `user_sp`: 用户栈顶地址
    /// - `kernel_sp`: 内核栈顶地址（从用户态 trap 回来时使用）
    /// - `argc`: 传递给用户程序的参数（写入 a0 寄存器）
    ///
    /// ## 教学概念：a0 = argc 的 ABI 约定
    ///
    /// RISC-V 调用约定（ILP32/LP64）中，a0（x10）既是第一个参数寄存器，
    /// 也是返回值寄存器。用户程序的 `_start` 入口点通过 a0 获取 argc。
    ///
    /// 对于简单的单程序内核，argc 通常为 0 或 1。
    /// 后续扩展为 argv 时，a0 = argc，a1 = argv 指针。
    pub fn set_user_trap_frame(&mut self, entry: usize, user_sp: usize, kernel_sp: usize, argc: usize) {
        // sstatus 设置：
        // - SPP (bit 8) = 0: sret 后进入 U-mode
        // - SPIE (bit 5) = 1: sret 后 SIE = 1（启用中断）
        // - SIE (bit 1) = 0: 当前中断关闭
        let sstatus: usize = 1 << 5; // SPIE=1, SPP=0, SIE=0
        self.sepc = entry;
        self.sstatus = sstatus;
        self.x2_sp = user_sp;
        self.kernel_sp = kernel_sp;
        // 清零其他寄存器（用户程序从干净状态开始）
        self.x1_ra = 0;
        self.x3_gp = 0;
        self.x4_tp = 0;
        self.x5_t0 = 0;
        self.x6_t1 = 0;
        self.x7_t2 = 0;
        self.x8_s0 = 0;
        self.x9_s1 = 0;
        self.x10_a0 = argc; // 用户程序通过 a0 获取 argc
        self.x11_a1 = 0;
        self.x12_a2 = 0;
        self.x13_a3 = 0;
        self.x14_a4 = 0;
        self.x15_a5 = 0;
        self.x16_a6 = 0;
        self.x17_a7 = 0;
        self.x18_s2 = 0;
        self.x19_s3 = 0;
        self.x20_s4 = 0;
        self.x21_s5 = 0;
        self.x22_s6 = 0;
        self.x23_s7 = 0;
        self.x24_s8 = 0;
        self.x25_s9 = 0;
        self.x26_s10 = 0;
        self.x27_s11 = 0;
        self.x28_t3 = 0;
        self.x29_t4 = 0;
        self.x30_t5 = 0;
        self.x31_t6 = 0;
    }
}

// ============================================================================
// enter_user_mode — 从 S-mode 切换到 U-mode
// ============================================================================

/// 从 S-mode 切换到 U-mode
///
/// 设置用户态陷阱帧后，通过 `trap_return` 执行 `sret` 切换到用户态。
///
/// ## 教学概念：特权级切换的完整流程
///
/// ```text
/// S-mode (内核)                          U-mode (用户)
/// ┌─────────────────┐                   ┌─────────────────┐
/// │ 1. 准备 TrapFrame│                   │ 用户程序从       │
/// │    SPP=User      │                   │ entry 开始执行   │
/// │    sepc=entry    │                   │                  │
/// │    SPIE=1        │                   │                  │
/// │                  │                   │                  │
/// │ 2. 写 sscratch   │                   │                  │
/// │    (TrapFrame指针)│                   │                  │
/// │                  │                   │                  │
/// │ 3. trap_return() │  ─── sret ───►    │                  │
/// │    恢复寄存器     │   SPP=0→U-mode    │                  │
/// │    sret          │   sepc→entry      │                  │
/// └─────────────────┘   SPIE→SIE=1      └─────────────────┘
/// ```
///
/// ## 安全性
///
/// - `entry` 必须是有效的用户态代码地址
/// - `user_sp` 必须是有效的用户栈顶地址
/// - `kernel_sp` 必须是有效的内核栈顶地址
///
/// # 参数
/// - `entry`: 用户程序入口地址
/// - `user_sp`: 用户栈顶地址
/// - `kernel_sp`: 内核栈顶地址（从用户态 trap 回来时切换到此栈）
/// - `argc`: 传递给用户程序的参数（写入 a0 寄存器）
pub fn enter_user_mode(entry: usize, user_sp: usize, kernel_sp: usize, argc: usize) -> ! {
    // 创建用户态陷阱帧
    let mut tf = TrapFrame::new();
    tf.set_user_trap_frame(entry, user_sp, kernel_sp, argc);

    // 将 TrapFrame 指针写入 sscratch（供下次从用户态 trap 回来时使用）
    // SAFETY: tf 是栈上有效的 TrapFrame，生命周期覆盖整个函数
    unsafe {
        asm!(
            "csrw sscratch, {tf_ptr}",
            tf_ptr = in(reg) &tf as *const TrapFrame as usize,
            options(nomem, nostack)
        );
    }

    // 通过 trap_return 执行 sret，切换到用户态
    // trap_return 在 trap.S 中实现：恢复全部寄存器，执行 sret
    // sret 硬件行为：PC ← sepc, SIE ← SPIE, 特权级 ← SPP
    // SAFETY: tf 是正确初始化的 TrapFrame，包含有效的 sstatus 和 sepc
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }

    // trap_return 不应返回（已切换到用户态）
    unreachable!()
}

// --- SyscallFrame trait 实现 ---

impl SyscallFrame for TrapFrame {
    /// 系统调用号在 a7 寄存器
    fn syscall_id(&self) -> usize {
        self.x17_a7
    }

    fn arg0(&self) -> usize { self.x10_a0 }
    fn arg1(&self) -> usize { self.x11_a1 }
    fn arg2(&self) -> usize { self.x12_a2 }
    fn arg3(&self) -> usize { self.x13_a3 }
    fn arg4(&self) -> usize { self.x14_a4 }
    fn arg5(&self) -> usize { self.x15_a5 }

    /// 返回值写入 a0 寄存器
    fn set_ret(&mut self, val: usize) {
        self.x10_a0 = val;
    }
}

// ============================================================================
// 内核页表初始化 — SV39 身份映射
// ============================================================================

use page::{PageTableEntry, PteFlags, PAGE_SIZE};

/// 内核根页表（512 个 PTE = 4KB）
///
/// 使用 `#[repr(C, align(4096))]` 确保 4KB 页对齐。
/// 这是 SV39 硬件要求：页表页必须页对齐。
///
/// ## 教学概念：静态页表
///
/// 内核页表在启动时初始化一次，之后所有内核代码共享。
/// 使用 `static` 而非栈上分配，因为页表生命周期是整个内核运行期。
#[repr(C, align(4096))]
struct KernelPageTable {
    entries: [PageTableEntry; 512],
}

/// 内核根页表实例
///
/// 全零初始化，所有 PTE 初始为无效（V=0）。
/// `init_kernel_page_table()` 会填充必要的映射。
static mut KERNEL_PAGE_TABLE: KernelPageTable = KernelPageTable {
    entries: [PageTableEntry::empty(); 512],
};

/// 初始化内核页表并激活 SV39 分页
///
/// 使用 **身份映射**（VA = PA）：虚拟地址和物理地址相同。
/// 这是最简单的映射方式，适合教学内核。
///
/// ## 教学概念：1GB 大页 (Superpage)
///
/// SV39 支持三级页表，但也可以在 Level 2 直接映射大页：
/// - Level 2 叶子 PTE → 1GB 页面（跳过 Level 1 和 Level 0）
/// - Level 1 叶子 PTE → 2MB 页面（跳过 Level 0）
/// - Level 0 叶子 PTE → 4KB 页面（标准）
///
/// 我们使用 1GB 大页，只需 4 个 PTE 就能映射全部 4GB 地址空间。
///
/// ## QEMU virt 内存布局
///
/// ```text
/// 0x0000_0000 - 0x4000_0000  (1GB): MMIO (UART, CLINT, PLIC)
/// 0x4000_0000 - 0x8000_0000  (1GB): 保留
/// 0x8000_0000 - 0xC000_0000  (1GB): DRAM (内核在此)
/// 0xC000_0000 - 0x1_0000_0000 (1GB): 保留
/// ```
pub fn init_kernel_page_table() {
    // 使用 1GB 大页进行身份映射
    // flags: V=1, R=1, W=1, X=1, A=1, D=1, G=1
    // - V: 有效
    // - R/W/X: 可读写执行（叶子 PTE 必须至少设置一个）
    // - A/D: 已访问/已脏（预设，避免硬件/软件设置问题）
    // - G: 全局映射（不随 ASID 刷新）
    let flags = PteFlags::VALID
        | PteFlags::READ
        | PteFlags::WRITE
        | PteFlags::EXECUTE
        | PteFlags::ACCESSED
        | PteFlags::DIRTY
        | PteFlags::GLOBAL;

    // Level 2 页表有 512 个条目，每个覆盖 1GB
    // index 0: 0x0000_0000 - 0x3FFF_FFFF (MMIO)
    // index 1: 0x4000_0000 - 0x7FFF_FFFF (保留)
    // index 2: 0x8000_0000 - 0xBFFF_FFFF (DRAM)
    // index 3: 0xC000_0000 - 0xFFFF_FFFF (保留)
    //
    // 我们映射 index 0-3，覆盖完整 4GB 物理地址空间。
    // 这样所有 MMIO 和 DRAM 都可访问。
    unsafe {
        for i in 0..4 {
            let pa_base = i * (1 << 30); // 每个条目 1GB = 2^30 字节
            let ppn = pa_base >> 12;     // PPN = PA / 4096
            KERNEL_PAGE_TABLE.entries[i] = PageTableEntry::new_leaf(ppn as u64, PteFlags(flags));
        }
    }

    // 写入 satp 寄存器激活 SV39 分页
    // satp 格式：MODE[63:60] | ASID[59:44] | PPN[43:0]
    // - MODE = 8: SV39 模式
    // - ASID = 0: 地址空间 ID（单地址空间）
    // - PPN: 根页表的物理页号
    let root_ppn = (&raw const KERNEL_PAGE_TABLE as *const _ as usize) >> 12;
    let satp_val: usize = (8_usize << 60) | root_ppn;

    // SAFETY: 写入 satp 是特权操作，必须在 S-mode 执行
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp_val,
        );
    }
}

/// 刷新 TLB（全部条目）
///
/// ## 教学概念：TLB 与 sfence.vma
///
/// TLB (Translation Lookaside Buffer) 缓存了最近的地址翻译结果。
/// 修改页表后，旧的翻译可能还在 TLB 中，必须手动刷新。
///
/// `sfence.vma` 指令刷新 TLB：
/// - `sfence.vma`: 刷新所有 TLB 条目
/// - `sfence.vma vaddr, zero`: 刷新特定虚拟地址
/// - `sfence.vma zero, asid`: 刷新特定 ASID
pub fn flush_tlb_all() {
    // SAFETY: sfence.vma 是特权指令
    unsafe {
        asm!("sfence.vma");
    }
}

/// 刷新特定虚拟地址的 TLB 条目
pub fn flush_tlb_addr(addr: usize) {
    // SAFETY: sfence.vma 是特权指令
    unsafe {
        asm!("sfence.vma {addr}, zero", addr = in(reg) addr);
    }
}

/// 读取当前 satp 寄存器值
pub fn read_satp() -> usize {
    let satp: usize;
    // SAFETY: satp 是只读 CSR（在 S-mode 下）
    unsafe {
        asm!("csrr {}, satp", out(reg) satp, options(nomem, nostack));
    }
    satp
}

/// 切换到新的页表
///
/// # Safety
///
/// `root_ppn` 必须指向一个有效的 SV39 页表。
pub unsafe fn switch_page_table(root_ppn: usize) {
    let satp_val: usize = (8_usize << 60) | root_ppn;
    // SAFETY: 切换页表是特权操作
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp_val,
        );
    }
}

// ============================================================================
// ELF 段加载
// ============================================================================

/// 将 ELF PT_LOAD 段加载到用户地址空间
///
/// 遍历所有 PT_LOAD 段，为每个段：
/// 1. 分配物理帧（`alloc_zeroed_frame`）
/// 2. 从 ELF 数据复制文件内容到物理帧（内核身份映射，直接写物理地址）
/// 3. 处理 BSS 段（`p_memsz > p_filesz` 的部分已由 `alloc_zeroed_frame` 清零）
/// 4. 将物理帧映射到用户地址空间（`map_user_page`，自动添加 U 标志）
///
/// ## 教学概念：加载过程
///
/// ```text
/// ELF 文件                    物理内存                  用户虚拟地址空间
/// ┌──────────┐               ┌──────────┐              ┌──────────┐
/// │ .text    │  ──copy──→    │ frame 0  │  ──map──→    │ 0x10000  │ R+X
/// │ .rodata  │  ──copy──→    │ frame 1  │  ──map──→    │ 0x11000  │ R
/// │ .data    │  ──copy──→    │ frame 2  │  ──map──→    │ 0x12000  │ R+W
/// │ .bss     │  ──zero──→    │ frame 3  │  ──map──→    │ 0x13000  │ R+W
/// └──────────┘               └──────────┘              └──────────┘
/// ```
///
/// # 参数
/// - `elf_data`: ELF 文件的原始字节
/// - `phdrs`: 已解析的程序头列表
/// - `user_space`: 用户地址空间（页表）
///
/// # 返回值
/// 成功返回 `Ok(())`，失败返回 `Err(())`。
pub fn load_segments(
    elf_data: &[u8],
    phdrs: &[suba_kernel::loader::ProgramHeader64],
    user_space: &page::UserAddrSpace,
) -> Result<(), ()> {
    use page::{PteFlags, PAGE_SIZE, alloc_zeroed_frame};
    use suba_kernel::loader::segment_flags;

    for phdr in phdrs {
        if !phdr.is_load() {
            continue;
        }

        let vaddr = phdr.vaddr();
        let file_size = phdr.file_size();
        let mem_size = phdr.mem_size();
        let offset = phdr.offset();

        // p_filesz 不能超过 p_memsz
        if file_size > mem_size {
            return Err(());
        }

        // 转换 ELF 权限标志为 PTE 标志
        let flags = elf_flags_to_pte_flags(phdr.p_flags);

        // 逐页处理：分配帧 → 复制数据 → 映射
        let num_pages = mem_size.div_ceil(PAGE_SIZE);

        for i in 0..num_pages {
            let page_va = vaddr + i * PAGE_SIZE;
            let page_file_offset = i * PAGE_SIZE;

            // 分配物理帧并清零（BSS 部分自然为零）
            let ppn = alloc_zeroed_frame().ok_or(())?;
            let pa = ppn * PAGE_SIZE;

            // 从 ELF 数据复制文件内容到物理帧
            if page_file_offset < file_size {
                let src_start = offset + page_file_offset;
                let copy_len = core::cmp::min(PAGE_SIZE, file_size - page_file_offset);
                let src_end = src_start + copy_len;

                // 边界检查
                if src_end > elf_data.len() {
                    return Err(());
                }

                // SAFETY: pa 是刚分配的物理帧，内核使用身份映射（VA=PA）
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_data[src_start..src_end].as_ptr(),
                        pa as *mut u8,
                        copy_len,
                    );
                }
            }
            // p_memsz > p_filesz 的部分保持为零（alloc_zeroed_frame 已清零）

            // 映射到用户地址空间（自动添加 U 标志）
            // SAFETY: pa 指向刚分配的有效物理帧
            unsafe {
                user_space.map_user_page(page_va, pa, flags).map_err(|_| ())?;
            }
        }
    }

    Ok(())
}

/// 从 ELF 权限标志转换为 PTE 标志
///
/// ELF 使用 PF_R(4) / PF_W(2) / PF_X(1)，
/// PTE 使用 READ(1<<1) / WRITE(1<<2) / EXECUTE(1<<3)。
fn elf_flags_to_pte_flags(elf_flags: u32) -> page::PteFlags {
    use suba_kernel::loader::segment_flags;

    let mut bits = page::PteFlags::VALID;
    if elf_flags & segment_flags::PF_R != 0 {
        bits |= page::PteFlags::READ;
    }
    if elf_flags & segment_flags::PF_W != 0 {
        bits |= page::PteFlags::WRITE;
    }
    if elf_flags & segment_flags::PF_X != 0 {
        bits |= page::PteFlags::EXECUTE;
    }
    page::PteFlags(bits)
}

// ============================================================================
// ELF 加载器实现 (ElfLoader trait)
// ============================================================================

/// RISC-V 64 位 ELF 加载器
///
/// 实现 kernel crate 的 [`suba_kernel::loader::ElfLoader`] trait，
/// 整合 ELF 解析、段加载和用户栈映射为一个完整的加载流程。
///
/// ## 教学概念：完整的 ELF 加载流程
///
/// 从 ELF 字节数据到可执行的用户程序，需要 5 个步骤：
///
/// ```text
/// ELF 数据 (字节切片)
///   │
///   ├─ Step 1: parse_elf_header()
///   │    验证魔数、64位、小端、RISC-V、可执行
///   │    提取 entry_point 和程序头表位置
///   │
///   ├─ Step 2: parse_program_headers()
///   │    遍历程序头表，解析每个段描述符
///   │
///   ├─ Step 3: UserAddrSpace::new()
///   │    创建独立的 SV39 用户页表
///   │    复制内核映射到高地址区域
///   │
///   ├─ Step 4: load_segments()
///   │    为每个 PT_LOAD 段分配物理帧
///   │    复制文件内容，清零 BSS
///   │    映射到用户虚拟地址空间
///   │
///   ├─ Step 5: map_user_stack()
///   │    分配 8MB 用户栈
///   │    映射到 USER_STACK_TOP 向下增长
///   │
///   └─ 返回 (entry_point, user_stack_top)
/// ```
///
/// ## 教学概念：加载器的角色
///
/// 加载器是"静态文件"到"动态执行"的桥梁：
/// - 输入：ELF 文件的字节数据（来自文件系统/initrd）
/// - 输出：入口地址 + 栈顶地址（用于设置 TrapFrame 并 sret 到用户态）
///
/// 加载器不负责"执行"程序——它只准备好内存环境。
/// 执行由 `enter_user_mode()` 完成：设置 sepc=entry, sp=stack_top, sret。
pub struct Riscv64ElfLoader;

impl suba_kernel::loader::ElfLoader for Riscv64ElfLoader {
    /// 从 ELF 数据加载用户程序到新的地址空间
    ///
    /// 完整流程：解析 ELF → 创建用户页表 → 加载段 → 映射栈。
    ///
    /// # 参数
    /// - `data`: ELF 文件的原始字节
    ///
    /// # 返回值
    /// 成功返回 `(entry_point, user_stack_top)`：
    /// - `entry_point`: 用户程序入口虚拟地址（写入 sepc）
    /// - `user_stack_top`: 用户栈顶虚拟地址（写入 x2_sp）
    ///
    /// 失败返回 `Err(())`，可能原因：
    /// - ELF 头验证失败（魔数、架构、类型）
    /// - 程序头表超出数据范围
    /// - 物理帧分配失败
    /// - 段数据超出 ELF 文件范围
    fn load_elf(data: &[u8]) -> Result<(usize, usize), ()> {
        use suba_kernel::loader::{parse_elf_header, parse_program_headers};

        // Step 1: 解析并验证 ELF 头
        // 提取入口点地址和程序头表位置
        let (entry, phoff, phentsize, phnum) = parse_elf_header(data).map_err(|_| ())?;

        // Step 2: 解析所有程序头
        // 遍历程序头表，收集每个段的描述信息
        let phdrs = parse_program_headers(data, phoff, phentsize, phnum).map_err(|_| ())?;

        // Step 3: 创建用户地址空间
        // 分配新的 SV39 根页表，复制内核映射到高地址
        let user_space = page::UserAddrSpace::new().map_err(|_| ())?;

        // Step 4: 加载 PT_LOAD 段
        // 为每个可加载段分配物理帧、复制数据、映射到用户页表
        load_segments(data, &phdrs, &user_space)?;

        // Step 5: 映射用户栈
        // 在 USER_STACK_TOP 向下分配 8MB 栈空间
        let user_stack_top = user_space.map_user_stack().map_err(|_| ())?;

        Ok((entry as usize, user_stack_top))
    }
}

/// 从 ELF 数据加载用户程序，返回完整的地址空间
///
/// 与 [`Riscv64ElfLoader::load_elf`] 类似，但额外返回 [`page::UserAddrSpace`]。
/// 调用者需要持有地址空间来切换页表（`user_space.activate()`）。
///
/// ## 教学概念：为什么需要这个函数？
///
/// `ElfLoader` trait 的 `load_elf` 只返回 `(entry, stack_top)`，
/// 但在实际内核启动流程中，还需要 `UserAddrSpace` 来：
/// 1. 切换 satp 到用户页表（`user_space.activate()`）
/// 2. 设置 sscratch 为 TrapFrame 指针
/// 3. 调用 `trap_return` 执行 sret
///
/// # 返回值
/// 成功返回 `(user_space, entry_point, user_stack_top)`。
pub fn load_elf_to_space(
    data: &[u8],
) -> Result<(page::UserAddrSpace, usize, usize), ()> {
    use suba_kernel::loader::{parse_elf_header, parse_program_headers};

    let (entry, phoff, phentsize, phnum) = parse_elf_header(data).map_err(|_| ())?;
    let phdrs = parse_program_headers(data, phoff, phentsize, phnum).map_err(|_| ())?;
    let user_space = page::UserAddrSpace::new().map_err(|_| ())?;
    load_segments(data, &phdrs, &user_space)?;
    let user_stack_top = user_space.map_user_stack().map_err(|_| ())?;

    Ok((user_space, entry as usize, user_stack_top))
}

// ============================================================================
// Riscv64Arch — Arch trait 完整实现
// ============================================================================

/// RISC-V 64 位架构实现
///
/// 实现 kernel crate 的 [`suba_kernel::arch::Arch`] trait，
/// 整合所有 RISC-V 后端组件：CPU 操作、内存管理、上下文切换、用户内存复制。
///
/// ## 教学概念：Arch trait 的角色
///
/// `Arch` 是内核与硬件之间的最高层抽象。内核其余代码通过 `Arch` trait：
/// - 切换任务上下文（`context_switch`）
/// - 复制用户空间数据（`copy_from_user` / `copy_to_user`）
/// - 获取架构信息（`name`, `cpu_count`）
///
/// 这使得内核核心代码完全与具体架构解耦，
/// 移植到新架构只需实现 `Arch` trait。
pub struct Riscv64Arch;

impl suba_kernel::arch::CpuOps for Riscv64Arch {
    fn id() -> usize {
        // SAFETY: 在多核系统中读取 mhartid 寄存器
        // 当前单核实现始终返回 0
        0
    }

    fn halt() -> ! {
        loop {
            // SAFETY: wfi 是特权指令，等待中断唤醒
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }

    fn enable_interrupts() {
        // SAFETY: 设置 sstatus.SIE = 1
        unsafe { core::arch::asm!("csrsi sstatus, 0x2", options(nomem, nostack)) }
    }

    fn disable_interrupts() -> usize {
        // 读取当前 sstatus 并禁用中断
        let old: usize;
        // SAFETY: 原子地读取 sstatus 并清除 SIE 位
        unsafe {
            core::arch::asm!(
                "csrrci {}, sstatus, 0x2",
                out(reg) old,
                options(nomem, nostack),
            );
        }
        old & 0x2 // 返回旧的 SIE 位
    }

    fn interrupts_enabled() -> bool {
        let sstatus: usize;
        // SAFETY: sstatus 是只读 CSR
        unsafe {
            core::arch::asm!("csrr {}, sstatus", out(reg) sstatus, options(nomem, nostack));
        }
        sstatus & 0x2 != 0
    }

    unsafe fn restore_interrupt_state(flags: usize) {
        if flags != 0 {
            // SAFETY: 恢复中断状态
            unsafe { core::arch::asm!("csrsi sstatus, 0x2", options(nomem, nostack)) }
        } else {
            unsafe { core::arch::asm!("csrci sstatus, 0x2", options(nomem, nostack)) }
        }
    }
}

impl suba_kernel::arch::MmOps for Riscv64Arch {
    unsafe fn translate_va(va: suba_kernel::arch::VA) -> Option<suba_kernel::arch::PA> {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF;
        match page::translate_va(va.as_usize(), root_ppn, page::phys_read_u64) {
            Ok(pa) => Some(suba_kernel::arch::PA::new(pa)),
            Err(_) => None,
        }
    }

    fn flush_tlb() {
        flush_tlb_all();
    }

    fn flush_tlb_addr(addr: usize) {
        flush_tlb_addr(addr);
    }

    fn current_page_table() -> suba_kernel::arch::PA {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF;
        suba_kernel::arch::PA::new(root_ppn << 12)
    }

    unsafe fn switch_page_table(pt_root: suba_kernel::arch::PA) {
        let root_ppn = pt_root.as_usize() >> 12;
        // SAFETY: 调用者确保 pt_root 指向有效页表
        unsafe {
            switch_page_table(root_ppn);
        }
    }
}

impl suba_kernel::arch::Arch for Riscv64Arch {
    type TrapFrame = TrapFrame;

    fn name() -> &'static str {
        "riscv64"
    }

    fn cpu_count() -> usize {
        1 // 当前单核实现
    }

    /// 上下文切换
    ///
    /// 调用 switch.S 中的汇编代码，保存 callee-saved 寄存器到 old，
    /// 从 new 恢复寄存器并跳转到 new.ra。
    ///
    /// ## 教学概念：自愿切换 vs 强制切换
    ///
    /// - **自愿切换** (context_switch): 任务主动让出 CPU（如 sys_yield）
    ///   只需保存 callee-saved 寄存器
    /// - **强制切换** (trap_return): 时钟中断抢占
    ///   需要保存全部寄存器（TrapFrame）
    ///
    /// # Safety
    ///
    /// `old` 和 `new` 必须指向有效的、正确对齐的 Context 结构体。
    unsafe fn context_switch(old: *mut suba_kernel::arch::Context, new: *const suba_kernel::arch::Context) {
        // SAFETY: 调用者确保 old/new 指向有效的 Context
        unsafe {
            switch(old, new);
        }
    }

    /// 从用户空间复制数据到内核空间
    ///
    /// 通过页表翻译用户虚拟地址，然后通过内核身份映射复制数据。
    ///
    /// ## 教学概念：用户/内核内存隔离
    ///
    /// 用户态和内核态使用不同的页表。内核不能直接解引用用户虚拟地址，
    /// 必须先通过页表翻译为物理地址，再通过内核的身份映射访问。
    ///
    /// ```text
    /// 用户 VA → [页表翻译] → 物理地址 → [内核身份映射] → 内核访问
    /// ```
    ///
    /// # Safety
    ///
    /// - `src` 必须是有效的用户虚拟地址
    /// - `dst` 必须指向足够大的内核缓冲区
    /// - 当前页表必须是用户页表（satp 指向用户页表）
    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
        if len == 0 {
            return Ok(());
        }
        if dst.is_null() {
            return Err(());
        }

        // 逐页复制：翻译每一页的用户 VA → PA，然后通过身份映射复制
        let mut remaining = len;
        let mut user_va = src;
        let mut kernel_dst = dst;

        while remaining > 0 {
            // 计算当前页内的偏移和可复制长度
            let page_offset = user_va & (page::PAGE_SIZE - 1);
            let copy_len = core::cmp::min(remaining, page::PAGE_SIZE - page_offset);

            // 翻译用户虚拟地址到物理地址
            // SAFETY: 我们正在处理用户地址空间
            let user_pa = translate_user_va(user_va)?;

            // 通过内核身份映射复制数据
            // SAFETY: user_pa 是有效的物理地址（页表翻译成功），
            // kernel_dst 是调用者确保有效的内核缓冲区
            unsafe {
                core::ptr::copy_nonoverlapping(
                    user_pa as *const u8,
                    kernel_dst,
                    copy_len,
                );
            }

            remaining -= copy_len;
            user_va += copy_len;
            // SAFETY: kernel_dst 在有效缓冲区内推进
            kernel_dst = unsafe { kernel_dst.add(copy_len) };
        }

        Ok(())
    }

    /// 从内核空间复制数据到用户空间
    ///
    /// 与 `copy_from_user` 对称：翻译用户 VA → PA，通过身份映射写入。
    ///
    /// # Safety
    ///
    /// - `dst` 必须是有效的用户虚拟地址（已映射且可写）
    /// - `src` 必须指向有效的内核数据
    /// - 当前页表必须是用户页表
    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
        if len == 0 {
            return Ok(());
        }
        if src.is_null() {
            return Err(());
        }

        let mut remaining = len;
        let mut user_va = dst;
        let mut kernel_src = src;

        while remaining > 0 {
            let page_offset = user_va & (page::PAGE_SIZE - 1);
            let copy_len = core::cmp::min(remaining, page::PAGE_SIZE - page_offset);

            // 翻译用户虚拟地址到物理地址
            let user_pa = translate_user_va(user_va)?;

            // 通过内核身份映射写入数据
            // SAFETY: user_pa 是有效的物理地址，kernel_src 是有效的内核数据
            unsafe {
                core::ptr::copy_nonoverlapping(
                    kernel_src,
                    user_pa as *mut u8,
                    copy_len,
                );
            }

            remaining -= copy_len;
            user_va += copy_len;
            // SAFETY: kernel_src 在有效数据范围内推进
            kernel_src = unsafe { kernel_src.add(copy_len) };
        }

        Ok(())
    }
}

/// 翻译用户虚拟地址到物理地址
///
/// 使用当前 satp 寄存器中的页表进行 SV39 三级页表遍历。
/// 返回物理地址，供内核身份映射使用。
///
/// # Safety
///
/// 当前 satp 必须指向包含该用户地址映射的页表。
fn translate_user_va(va: usize) -> Result<usize, ()> {
    let satp = read_satp();
    let root_ppn = satp & 0x0FFF_FFFF_FFFF;
    page::translate_va(va, root_ppn, page::phys_read_u64).map_err(|_| ())
}
