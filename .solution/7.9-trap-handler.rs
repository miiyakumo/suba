// .solution/7.9-trap-handler.rs — trap_handler 中断分发参考实现
//
// 本文件展示如何在 trap_handler 中根据 scause 分发中断/异常处理。

/// trap_handler — 陷阱处理函数（由 trap.S 调用）
///
/// 读取 scause 寄存器判断陷阱类型，分发到对应的处理逻辑。
///
/// ## 教学概念：RISC-V 陷阱分发
///
/// scause 寄存器编码陷阱原因：
/// - 最高位 (bit 63) = 1: 中断（异步事件）
/// - 最高位 = 0: 异常（同步事件，由当前指令触发）
/// - 低位 = 具体原因编号
///
/// 常见 scause 值：
/// - 8: Environment call from U-mode（用户态系统调用）
/// - (1<<63)|5: Supervisor timer interrupt（时钟中断）
/// - (1<<63)|9: Supervisor external interrupt（外部设备中断）
///
/// # Safety
/// 必须从 trap.S 中以正确的 TrapFrame 指针调用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    // 读取 scause 寄存器
    let scause = read_scause();

    // 最高位区分中断 vs 异常
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ---- 中断处理 ----
        match scause & !INTERRUPT_BIT {
            5 => {
                // Supervisor timer interrupt（时钟中断）
                // 设置下一次时钟中断
                set_next_timer();
                // TODO: 时间片调度（后续 feature）
            }
            9 => {
                // Supervisor external interrupt（外部设备中断，如 UART）
                // TODO: PLIC 中断处理（后续 feature）
            }
            _ => {
                // 未知中断
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
                // TODO: 调用系统调用分发（后续 feature）
                // dispatch_syscall(tf);
            }
            _ => {
                // 未知异常
                let stval = read_stval();
                panic!(
                    "[suba] unexpected exception: scause={:#x}, stval={:#x}, sepc={:#x}",
                    scause, stval, tf.sepc
                );
            }
        }
    }

    // 处理完毕，恢复寄存器并返回
    // SAFETY: trap_frame 指向有效的 TrapFrame（由 trap_entry 保存）
    unsafe {
        trap_return(trap_frame);
    }
}

// ---------------------------------------------------------------------------
// CSR 辅助函数
// ---------------------------------------------------------------------------

/// 读取 scause 寄存器（陷阱原因）
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
#[inline]
fn read_stval() -> usize {
    let stval: usize;
    // SAFETY: stval 是只读 CSR
    unsafe {
        asm!("csrr {}, stval", out(reg) stval, options(nomem, nostack));
    }
    stval
}

/// 设置下一次时钟中断
///
/// 通过 SBI 调用 `set_timer` 设置比较器。
/// 当 mtime >= stimecmp 时触发时钟中断。
fn set_next_timer() {
    let current_time: u64;
    // SAFETY: time 是只读 CSR
    unsafe {
        asm!("csrr {}, time", out(reg) current_time, options(nomem, nostack));
    }
    const TIMER_INTERVAL: u64 = 10_000_000; // ~100ms @ 10MHz
    sbi_rt::set_timer(current_time + TIMER_INTERVAL);
}
