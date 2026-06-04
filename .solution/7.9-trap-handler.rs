// .solution/7.9-trap-handler.rs — trap_handler 陷阱分发参考实现
//
// 本文件是 feature 7.9 的完整参考实现。
// 学生应在 os/src/arch/riscv64/mod.rs 中的 TODO(student) 处自行实现。

// ============================================================================
// CSR 读取辅助函数（提供给学生使用）
// ============================================================================

/// 读取 scause CSR 寄存器
///
/// scause 记录陷阱原因：
/// - 最高位 (bit 63) = 1: 中断 (interrupt)
/// - 最高位 (bit 63) = 0: 异常 (exception)
/// - 低 bits: 具体原因编号
#[inline]
fn read_scause() -> usize {
    let scause: usize;
    unsafe {
        core::arch::asm!("csrr {}, scause", out(reg) scause, options(nomem, nostack));
    }
    scause
}

/// 读取 stval CSR 寄存器
///
/// stval 提供陷阱的附加信息：
/// - 页错误: 访问的虚拟地址
/// - 非法指令: 指令编码
/// - 地址未对齐: 访问的地址
#[inline]
fn read_stval() -> usize {
    let stval: usize;
    unsafe {
        core::arch::asm!("csrr {}, stval", out(reg) stval, options(nomem, nostack));
    }
    stval
}

/// 读取 sepc CSR 寄存器
///
/// sepc 保存陷阱发生时的指令地址（即"断点"）。
/// sret 指令会将 sepc 写回 PC，从而恢复被中断的执行。
#[inline]
fn read_sepc() -> usize {
    let sepc: usize;
    unsafe {
        core::arch::asm!("csrr {}, sepc", out(reg) sepc, options(nomem, nostack));
    }
    sepc
}

/// 设置下一次时钟中断
///
/// 通过 SBI 调用 `set_timer` 设置比较器。
/// 当 mtime >= stimecmp 时触发时钟中断。
fn set_next_timer() {
    let current_time: u64;
    unsafe {
        core::arch::asm!("csrr {}, time", out(reg) current_time, options(nomem, nostack));
    }
    const TIMER_INTERVAL: u64 = 10_000_000; // ~100ms @ QEMU ~10MHz
    sbi_rt::set_timer(current_time + TIMER_INTERVAL);
}

// ============================================================================
// trap_handler — 陷阱分发函数（参考实现）
// ============================================================================

/// trap_handler — 陷阱分发函数
///
/// 关键步骤：
/// 1. 读取 scause 寄存器
/// 2. 检查最高位区分中断 vs 异常
/// 3. 根据具体原因编号分发处理
/// 4. 处理完毕后调用 trap_return 恢复执行
///
/// # Safety
/// 必须从 trap.S 中以正确的 TrapFrame 指针调用。
#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    // Step 1: 读取 scause
    let scause = read_scause();

    // Step 2: 最高位区分中断 vs 异常
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ---- 中断处理 ----
        match scause & !INTERRUPT_BIT {
            5 => {
                // 时钟中断: 设置下一次触发
                set_next_timer();
            }
            9 => {
                // 外部中断: 后续 feature 实现 PLIC
            }
            _ => {
                panic!("[suba] unexpected interrupt: scause={:#x}", scause);
            }
        }
    } else {
        // ---- 异常处理 ----
        match scause {
            8 => {
                // ecall from U-mode: 系统调用
                // 必须将 sepc + 4 跳过 ecall 指令！
                tf.sepc = tf.sepc.wrapping_add(4);
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

    // Step 3: 恢复寄存器并返回
    unsafe {
        trap_return(trap_frame);
    }
}
