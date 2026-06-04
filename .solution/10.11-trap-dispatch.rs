// Solution: 10.11 - 实现 RISC-V 中断分发完善
//
// 本文件展示完整的陷阱分发逻辑。
// 完整代码位于 os/src/arch/riscv64/mod.rs。

// ============================================================================
// trap_handler — 陷阱分发函数
// ============================================================================

/// trap_handler — 由 trap.S 调用的陷阱分发函数
///
/// ## 教学概念：RISC-V 陷阱分发 (Trap Dispatch)
///
/// RISC-V 的陷阱处理是"统一入口，分别处理"：
/// 1. 所有陷阱共享同一个入口地址（stvec → trap_entry）
/// 2. 软件读取 scause 判断陷阱类型
/// 3. 根据类型跳转到不同的处理函数
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    let scause = read_scause();
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ---- 中断处理 ----
        match scause & !INTERRUPT_BIT {
            5 => {
                // Supervisor timer interrupt（时钟中断）
                // CLINT 处理：重置 timer + 触发调度
                crate::driver::clint::handle_timer_interrupt();
            }
            9 => {
                // Supervisor external interrupt（外部设备中断）
                // PLIC 处理：读取中断源 + 分发到设备驱动
                crate::driver::plic::handle_external_interrupt();
            }
            _ => {
                panic!("[suba] unexpected interrupt: scause={:#x}", scause);
            }
        }
    } else {
        // ---- 异常处理 ----
        match scause {
            8 => {
                // Environment call from U-mode（用户态系统调用）
                // sepc 指向 ecall 指令，需要前进 4 字节到下一条指令
                tf.sepc = tf.sepc.wrapping_add(4);
                // 系统调用分发（后续 feature 完成）
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

    // 处理完毕，调用 trap_return 恢复寄存器并返回
    // trap_return 在 trap.S 中实现：恢复全部寄存器，执行 sret
    unsafe {
        trap_return(trap_frame);
    }
}
