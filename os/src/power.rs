/// 电源管理 — RISC-V 实现
///
/// 通过 SBI 调用实现关机和重启。
///
/// ## 教学概念：SBI 电源管理
///
/// RISC-V 的电源管理通过 SBI (Supervisor Binary Interface) 调用实现。
/// SBI 是运行在 M-mode 的固件（如 OpenSBI）提供的接口，
/// S-mode 内核通过 `ecall` 指令调用 SBI 函数。
///
/// 常用的 SBI 电源管理函数：
/// - `system_reset(Shutdown, NoReason)` — 正常关机
/// - `system_reset(Shutdown, SystemFailure)` — 故障关机
/// - `system_reset(Reboot, NoReason)` — 重启

use suba_kernel::driver::Power;

/// RISC-V 电源管理实现
///
/// 使用 `sbi_rt` crate 的 SBI 调用实现关机和重启。
pub struct Riscv64Power;

impl Power for Riscv64Power {
    /// 关机
    ///
    /// 通过 SBI `system_reset` 调用关闭系统。
    /// 在 QEMU virt 机器上，这会使 QEMU 进程退出。
    fn shutdown() -> ! {
        use sbi_rt::{NoReason, Shutdown, system_reset};
        system_reset(Shutdown, NoReason);
        // SBI 调用不应该返回，但如果返回了，死循环
        loop {
            // SAFETY: wfi 是特权指令
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }

    /// 重启
    ///
    /// 通过 SBI `system_reset` 调用重启系统。
    fn reboot() -> ! {
        use sbi_rt::{ColdReboot, NoReason, system_reset};
        system_reset(ColdReboot, NoReason);
        loop {
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }
}

/// 关机（兼容旧接口）
///
/// 调用 [`Riscv64Power::shutdown()`]。
///
/// ## 教学概念：完整关机流程
///
/// 内核关机不是简单调用 SBI shutdown，而是需要有序的收尾：
///
/// ```text
/// 1. 检测条件：所有用户任务 Exited
///    └─ all_user_tasks_exited() 遍历 TASK_MANAGER
///
/// 2. 资源回收 (TODO: student)
///    ├─ 遍历 TASK_MANAGER 回收所有任务的 Arc<Task>
///    ├─ 回收用户页表的物理帧
///    └─ 回收内核栈的物理帧
///
/// 3. 输出关机信息
///    └─ 打印每个任务的退出码摘要
///
/// 4. SBI 关机
///    └─ sbi_rt::system_reset(Shutdown, NoReason)
///       └─ OpenSBI 收到调用 → 执行关机 → QEMU 退出
/// ```
///
/// TODO(student): 关机流程完善
/// 当前实现直接调用 SBI 关机。完整实现应该：
/// 1. 在 schedule() 中检测 all_user_tasks_exited()
/// 2. 关机前回收所有任务资源（释放 Arc、物理帧）
/// 3. 打印关机摘要（每个任务的 PID 和退出码）
/// 4. 最后调用 shutdown(false) 关闭 QEMU
///
/// 参考：
/// - os/src/main.rs: all_user_tasks_exited(), schedule()
/// - .solution/11.11-shutdown.rs
pub fn shutdown(failure: bool) -> ! {
    if failure {
        // 故障关机：使用 SystemFailure
        use sbi_rt::{Shutdown, SystemFailure, system_reset};
        system_reset(Shutdown, SystemFailure);
    } else {
        Riscv64Power::shutdown();
    }
    unreachable!()
}
