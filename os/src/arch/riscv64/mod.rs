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
use suba_kernel::arch::{Context, CpuOps};

// 包含上下文切换汇编代码
core::arch::global_asm!(include_str!("switch.S"));

// RISC-V 上下文切换的 extern 汇编入口
// 在 switch.S 中实现，保存 callee-saved 寄存器到 old_ctx，
// 从 new_ctx 恢复寄存器并跳转。
//
// Safety: old_ctx 和 new_ctx 必须指向有效的、正确对齐的 Context 结构体。
unsafe extern "C" {
    pub fn switch(old_ctx: *mut Context, new_ctx: *const Context);
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
