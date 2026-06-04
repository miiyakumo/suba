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

/// trap_handler — 陷阱处理函数（由 trap_entry.S 调用）
///
/// 目前为空实现，后续 feature (7.9) 会填充 scause 分发逻辑。
///
/// # Safety
/// 必须从 trap_entry.S 中以正确的 TrapFrame 指针调用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(_trap_frame: *mut TrapFrame) {
    // TODO(feature 7.9): 根据 scause 分发中断/异常处理
    // 1. 读取 scause 寄存器判断陷阱类型
    // 2. 异常: ecall → 系统调用
    // 3. 中断: timer → 时钟中断, external → PLIC
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
