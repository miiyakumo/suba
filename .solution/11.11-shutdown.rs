// Solution: 11.11 - 内核关机和清理
//
// 本文件展示完整的内核关机流程：
// 检测所有任务退出 → 资源回收 → SBI 关机。
//
// 完整代码分布在以下文件中：
//   os/src/power.rs — SBI shutdown/reboot 实现
//   os/src/main.rs  — all_user_tasks_exited(), schedule() 关机检测

// ============================================================================
// power.rs — SBI 电源管理 (完整实现)
// ============================================================================

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

/// 关机（兼容旧接口，支持故障模式）
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
pub fn shutdown(failure: bool) -> ! {
    if failure {
        use sbi_rt::{Shutdown, SystemFailure, system_reset};
        system_reset(Shutdown, SystemFailure);
    } else {
        Riscv64Power::shutdown();
    }
    unreachable!()
}

// ============================================================================
// main.rs — 调度器中的关机检测
// ============================================================================

/// 检查是否所有用户任务已退出。
///
/// 遍历 TASK_MANAGER 中所有非 idle (PID > 1) 的任务，
/// 如果全部处于 Exited 状态，返回 true。
///
/// ## 教学概念：关机检测
///
/// 内核通过定时检查任务状态来决定何时关机：
/// - 当所有用户任务都退出（Exited）时，没有更多工作需要完成
/// - 内核进行最后一次资源回收，然后通过 SBI 调用关闭硬件
/// - 这避免了内核在 QEMU 中"挂死"——永远等待不存在的就绪任务
///
/// idle 任务 (PID=1) 永远处于 Running 状态，不参与检查。
fn all_user_tasks_exited() -> bool {
    if let Some(tm) = TASK_MANAGER.get() {
        let tm = tm.lock();
        let total = tm.len();
        // idle 任务 (PID=1) 总是 Running，只检查 PID >= 2 的用户任务
        for pid in 2..=total {
            if let Some(task) = tm.get_task(pid) {
                if task.lock().state != TaskState::Exited {
                    return false;
                }
            }
        }
        total > 1 // 至少有一个用户任务存在
    } else {
        false
    }
}

/// 调度器 — 选择下一个任务并切换。
///
/// 从调度器取出下一个就绪任务，通过上下文切换跳转到该任务。
/// 如果没有就绪任务且所有用户任务已退出，触发系统关机。
fn schedule() {
    let next_info = if let Some(sched) = SCHEDULER.get() {
        sched.lock().next().and_then(|task| {
            let mut t = task.lock();
            t.state = TaskState::Running;
            let pid = t.pid;
            CURRENT_PID.store(pid, Ordering::Relaxed);
            Some((t.context_ptr(), t.trap_frame_ptr, t.page_table_root))
        })
    } else {
        None
    };

    if let Some((next_ctx, trap_frame_ptr, page_table_root)) = next_info {
        if page_table_root != 0 {
            unsafe { switch_page_table(page_table_root); }
        }
        if trap_frame_ptr != 0 {
            NEXT_TRAP_FRAME.store(trap_frame_ptr as _, Ordering::Release);
        }
        let current_pid = CURRENT_PID.load(Ordering::Relaxed);
        let old_ctx = if let Some(tm) = TASK_MANAGER.get() {
            tm.lock().get_task(current_pid).map(|t| t.lock().context_ptr())
        } else { None };

        if let Some(old_ctx) = old_ctx {
            unsafe { Riscv64Arch::context_switch(old_ctx, next_ctx); }
        }
    } else {
        // 关机检测：所有用户任务退出时正常关机
        //
        // ## 教学概念：shutdown 触发条件
        //
        // ```text
        // 用户任务 A → exit → Exited → schedule() → 切换到任务 B
        // 用户任务 B → exit → Exited → schedule() → 无就绪任务
        //   → all_user_tasks_exited() = true
        //   → power::shutdown(false) → SBI → QEMU 退出
        // ```
        //
        // 关机前执行的工作：
        // 1. 打印关机消息到 UART
        // 2. 调用 SBI system_reset（QEMU 会退出并返回退出码）
        if all_user_tasks_exited() {
            uart_puts("[suba] all user tasks exited, shutting down...\n");
            power::shutdown(false);
        }
    }
}

// ============================================================================
// 教学概念总结：内核关机全流程
// ============================================================================

/// ```text
/// 用户任务生命周期 → 内核关机:
///
/// ┌───────────┐    ecall       ┌──────────────┐
/// │ Task A    │ ─────────────► │ trap_handler │
/// │ exit(0)   │                │ dispatch(tf) │
/// └───────────┘                │ sys_exit(0)  │
///                              │ after_syscall│
///                              │   mark Exited│
///                              │   schedule() │
///                              └──────┬───────┘
///                                     │
///                                     ▼
///                              ┌──────────────┐
///                              │ scheduler    │
///                              │   next() → B │
///                              │ context_switch│
///                              └──────┬───────┘
///                                     │ sret
///                                     ▼
/// ┌───────────┐    ecall       ┌──────────────┐
/// │ Task B    │ ─────────────► │ trap_handler │
/// │ exit(1)   │                │ dispatch(tf) │
/// └───────────┘                │ sys_exit(1)  │
///                              │ after_syscall│
///                              │   mark Exited│
///                              │   schedule() │
///                              └──────┬───────┘
///                                     │
///                                     ▼
///                              ┌──────────────┐
///                              │ scheduler    │
///                              │   next() → None│  ← 无就绪任务!
///                              └──────┬───────┘
///                                     │
///                                     ▼
///                              ┌──────────────┐
///                              │ all_tasks    │
///                              │   _exited()  │
///                              │   → true     │
///                              └──────┬───────┘
///                                     │
///                                     ▼
///                              ┌──────────────┐
///                              │ power::      │
///                              │ shutdown     │
///                              │ SBI system   │
///                              │   _reset     │
///                              └──────┬───────┘
///                                     │
///                                     ▼
///                              ┌──────────────┐
///                              │ QEMU 退出     │
///                              │ (exit code 0) │
///                              └──────────────┘
/// ```

// ============================================================================
// 边界条件分析
// ============================================================================

/// 1. **只有 idle 任务，无用户任务**: all_user_tasks_exited() = false (total=1 不满足 total>1)
///
/// 2. **任务 A 在 Running 状态但不在调度队列**: all_user_tasks_exited() = false (A 不是 Exited)
///    正确行为：不关机，等待 A 完成或下一个定时器中断
///
/// 3. **所有任务已退出但恐慌**: panic_handler 调用 power::shutdown(false)
///    正确行为：直接关机（失败模式使用 SystemFailure）
///
/// 4. **关机后 schedule() 继续执行**: power::shutdown 的返回类型是 `!`
///    Rust 编译器保证调用 shutdown 后不会继续执行
///
/// 5. **多 hart 场景**: 当前 suba 是单核实现（hart 0 唯一）
///    多核场景需要等待所有 hart 完成或发送 IPI 协调关机
