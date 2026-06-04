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

use suba_kernel::arch::Context;

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
