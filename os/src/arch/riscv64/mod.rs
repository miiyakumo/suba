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
use suba_kernel::arch::{Context, CpuOps, HwTrapFrame, SyscallFrame};

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
/// scause 寄存器编码陷阱原因：
/// - 最高位 (bit 63) = 1: 中断（异步事件，由硬件触发）
/// - 最高位 = 0: 异常（同步事件，由当前指令触发）
/// - 低位 = 具体原因编号
///
/// 常见 scause 值：
/// | scause       | 类型 | 含义                     |
/// |--------------|------|--------------------------|
/// | 8            | 异常 | 用户态 ecall（系统调用）  |
/// | (1<<63) \| 5 | 中断 | 时钟中断 (timer)         |
/// | (1<<63) \| 9 | 中断 | 外部中断 (PLIC)          |
///
/// # Safety
/// 必须从 trap.S 中以正确的 TrapFrame 指针调用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    // TODO(student): 读取 scause 寄存器，判断陷阱类型
    // 提示: 调用 read_scause() 函数
    // scause 最高位区分中断 (1) vs 异常 (0)
    let scause = read_scause();

    // TODO(student): 根据 scause 的最高位，分发到中断处理或异常处理
    // 提示:
    //   - 最高位 (bit 63) 是中断标志位：scause & (1 << 63) != 0 → 中断
    //   - 中断编号 = scause & !(1 << 63)（去掉最高位）
    //   - 异常编号 = scause 本身（最高位已经是 0）
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ---- 中断处理 ----
        // TODO(student): 根据中断编号分发
        // - 5: 时钟中断 → 调用 set_next_timer()
        // - 9: 外部中断 → 暂时打印信息（后续 feature 实现 PLIC）
        // - 其他: panic
        match scause & !INTERRUPT_BIT {
            5 => {
                // Supervisor timer interrupt（时钟中断）
                set_next_timer();
                // TODO: 时间片调度（后续 feature）
            }
            9 => {
                // Supervisor external interrupt（外部设备中断）
                // TODO: PLIC 中断处理（后续 feature）
            }
            _ => {
                panic!("[suba] unexpected interrupt: scause={:#x}", scause);
            }
        }
    } else {
        // ---- 异常处理 ----
        // TODO(student): 根据异常编号分发
        // - 8: ecall from U-mode → 系统调用
        //   重要: 需要将 sepc + 4 跳过 ecall 指令，否则 sret 后会重新执行 ecall！
        // - 其他: panic（打印 scause, stval, sepc 信息）
        match scause {
            8 => {
                // Environment call from U-mode（用户态系统调用）
                // sepc 指向 ecall 指令，需要前进 4 字节到下一条指令
                tf.sepc = tf.sepc.wrapping_add(4);
                // TODO: 调用系统调用分发（后续 feature）
                // dispatch_syscall(tf);
            }
            _ => {
                let stval = read_stval();
                panic!(
                    "[suba] unexpected exception: scause={:#x}, stval={:#x}, sepc={:#x}",
                    scause, stval, tf.sepc
                );
            }
        }
    }

    // TODO(student): 处理完毕后，调用 trap_return 恢复寄存器并返回
    // trap_return 在 trap.S 中实现，会从 TrapFrame 恢复所有寄存器并执行 sret
    // 提示: trap_return(trap_frame)
    unsafe {
        trap_return(trap_frame);
    }
}

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
pub unsafe fn drop_to_user_mode(user_entry: usize, user_sp: usize, kernel_sp: usize) {
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
) {
    tf.sepc = user_entry;
    // SPP=0 (User), SPIE=1, SIE=0
    tf.sstatus = 1 << 5;
    tf.x2_sp = user_sp;
    tf.kernel_sp = kernel_sp;
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

/// 设置下一次时钟中断
///
/// 通过 SBI 调用 `set_timer` 设置比较器。
/// 当 mtime >= stimecmp 时触发时钟中断。
///
/// ## 教学概念：RISC-V 时钟机制
///
/// RISC-V 使用 memory-mapped 的 mtime 寄存器（单调递增计数器）
/// 和 stimecmp 寄存器（比较值）来产生时钟中断。
/// 当 mtime >= stimecmp 时，触发 Supervisor timer interrupt。
///
/// 在 QEMU 中，mtime 频率约为 10MHz。
/// 我们设置 100ms 的间隔（10_000_000 个 tick）。
fn set_next_timer() {
    // 读取当前时间
    let current_time: u64;
    // SAFETY: time 是只读 CSR（RDCYCLE 等效）
    unsafe {
        asm!(
            "csrr {}, time",
            out(reg) current_time,
            options(nomem, nostack)
        );
    }
    // 设置下一次时钟中断（100ms 后，QEMU ~10MHz）
    const TIMER_INTERVAL: u64 = 10_000_000;
    sbi_rt::set_timer(current_time + TIMER_INTERVAL);
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
    pub fn set_user_trap_frame(&mut self, entry: usize, user_sp: usize, kernel_sp: usize) {
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
        self.x10_a0 = 0;
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
pub fn enter_user_mode(entry: usize, user_sp: usize, kernel_sp: usize) -> ! {
    // 创建用户态陷阱帧
    let mut tf = TrapFrame::new();
    tf.set_user_trap_frame(entry, user_sp, kernel_sp);

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
