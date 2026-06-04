//! RISC-V CpuOps 参考实现
//!
//! Riscv64CpuOps 通过 CSR 指令实现 CpuOps trait。
//!
//! ## CSR 常量
//!
//! const SSTATUS_SIE: usize = 1 << 1;  // S-mode Interrupt Enable
//!
//! ## 实现要点
//!
//! - id(): csrr rd, mhartid — 读取硬件线程 ID
//! - halt(): wfi 循环 — 低功耗等待中断
//! - disable_interrupts(): csrrc rd, sstatus, mask — 原子清除 SIE 并返回旧值
//! - restore_interrupt_state(flags): 检查旧 SIE，若为 1 则重新开启
//! - enable_interrupts(): csrs sstatus, mask — 原子设置 SIE
//! - interrupts_enabled(): csrr rd, sstatus — 读取并检查 SIE 位
