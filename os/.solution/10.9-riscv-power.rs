// =============================================================================
// Solution: 10.9 — 实现 RISC-V Power trait
// =============================================================================
//
// 本文件展示如何为 RISC-V 实现 Power trait，通过 SBI 调用实现关机和重启。
//
// ## 核心概念
//
// RISC-V 的电源管理通过 SBI (Supervisor Binary Interface) 调用实现。
// SBI 是运行在 M-mode 的固件（如 OpenSBI）提供的接口，
// S-mode 内核通过 `ecall` 指令调用 SBI 函数。
//
// ## SBI 电源管理函数
//
// - `system_reset(Shutdown, NoReason)` — 正常关机
// - `system_reset(Shutdown, SystemFailure)` — 故障关机
// - `system_reset(Reboot, NoReason)` — 重启
//
// ## 实现要点
//
// ### 1. Riscv64Power 结构体
//
// ```rust
// pub struct Riscv64Power;
// ```
//
// 这是一个零大小的标记类型，用于实现 Power trait。
//
// ### 2. shutdown 实现
//
// ```rust
// fn shutdown() -> ! {
//     use sbi_rt::{NoReason, Shutdown, system_reset};
//     system_reset(Shutdown, NoReason);
//     // SBI 调用不应该返回，但如果返回了，死循环
//     loop {
//         unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
//     }
// }
// ```
//
// ### 3. reboot 实现
//
// ```rust
// fn reboot() -> ! {
//     use sbi_rt::{ColdReboot, NoReason, system_reset};
//     system_reset(ColdReboot, NoReason);
//     loop {
//         unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
//     }
// }
// ```
//
// ## 教学概念：SBI 调用
//
// SBI 是 RISC-V 的固件接口规范。OpenSBI 是最常见的实现，
// 运行在 M-mode，为 S-mode 内核提供服务：
//
// ```text
// ┌─────────────────────────────────────┐
// │  S-mode (内核)                      │
// │  ecall → SBI 调用                   │
// ├─────────────────────────────────────┤
// │  M-mode (OpenSBI)                   │
// │  处理 SBI 请求，操作硬件             │
// └─────────────────────────────────────┘
// ```
//
// ## 参考实现
//
// comix 的 shutdown 实现：
//
// ```rust
// pub fn shutdown(failure: bool) -> ! {
//     use sbi_rt::{NoReason, Shutdown, SystemFailure, system_reset};
//     if !failure {
//         system_reset(Shutdown, NoReason);
//     } else {
//         system_reset(Shutdown, SystemFailure);
//     }
//     unreachable!()
// }
// ```
//
// ## 关键点
//
// 1. **SBI 调用**: 使用 `sbi_rt` crate，它是 SBI 规范的 Rust 绑定
//
// 2. **不可返回**: `shutdown()` 和 `reboot()` 返回类型是 `!`（never type），
//    表示函数永远不会返回
//
// 3. **WFI 兜底**: 如果 SBI 调用意外返回，使用 wfi 指令进入低功耗等待
//
// 4. **QEMU 行为**: 在 QEMU virt 机器上，system_reset 会使 QEMU 进程退出
