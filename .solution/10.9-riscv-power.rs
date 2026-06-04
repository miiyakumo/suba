// Solution: 10.9 - 实现 RISC-V Power trait
//
// 本文件展示如何为 RISC-V 实现 Power trait。
// 完整代码位于 os/src/power.rs。

use suba_kernel::driver::Power;

/// RISC-V 电源管理实现
///
/// 使用 `sbi_rt` crate 的 SBI 调用实现关机和重启。
///
/// ## 教学概念：SBI 电源管理
///
/// RISC-V 的电源管理通过 SBI (Supervisor Binary Interface) 调用实现。
/// SBI 是运行在 M-mode 的固件（如 OpenSBI）提供的接口，
/// S-mode 内核通过 `ecall` 指令调用 SBI 函数。
///
/// `sbi_rt` crate 封装了 SBI 调用，提供类型安全的 Rust API：
/// - `system_reset(Shutdown, NoReason)` — 正常关机
/// - `system_reset(ColdReboot, NoReason)` — 冷重启
/// - `system_reset(Shutdown, SystemFailure)` — 故障关机
pub struct Riscv64Power;

impl Power for Riscv64Power {
    fn shutdown() -> ! {
        use sbi_rt::{NoReason, Shutdown, system_reset};
        system_reset(Shutdown, NoReason);
        loop {
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }

    fn reboot() -> ! {
        use sbi_rt::{ColdReboot, NoReason, system_reset};
        system_reset(ColdReboot, NoReason);
        loop {
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }
}
