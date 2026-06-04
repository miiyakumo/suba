// Solution: 10.11 - 实现 RISC-V 中断分发完善
//
// 本文件展示完整的陷阱分发逻辑。
// 完整代码位于 os/src/arch/riscv64/mod.rs。
//
// ## 完整的陷阱分发表
//
// | scause          | 类型 | 含义                    | 处理函数                    |
// |-----------------|------|-------------------------|-----------------------------|
// | 0               | 异常 | 指令地址未对齐          | panic                       |
// | 2               | 异常 | 非法指令                | panic                       |
// | 5               | 异常 | 加载地址未对齐          | panic                       |
// | 7               | 异常 | 存储地址未对齐          | panic                       |
// | 8               | 异常 | U-mode ecall            | sepc+=4, dispatch_syscall   |
// | 12              | 异常 | 指令页错误              | panic                       |
// | 13              | 异常 | 加载页错误              | panic                       |
// | 15              | 异常 | 存储页错误              | panic                       |
// | (1<<63)\|1      | 中断 | 软件中断 (SSI)          | (当前忽略)                  |
// | (1<<63)\|5      | 中断 | 时钟中断 (STI)          | clint::handle_timer         |
// | (1<<63)\|9      | 中断 | 外部中断 (SEI)          | plic::handle_external       |

#[unsafe(no_mangle)]
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    // SAFETY: trap_frame 由 trap.S 传入，指向有效的 TrapFrame
    let tf = unsafe { &mut *trap_frame };

    let scause = read_scause();
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // ---- 中断处理 ----
        match scause & !INTERRUPT_BIT {
            1 => {
                // Supervisor software interrupt (SSI) — 当前忽略
            }
            5 => {
                // Supervisor timer interrupt (STI)
                crate::driver::clint::handle_timer_interrupt();
            }
            9 => {
                // Supervisor external interrupt (SEI)
                crate::driver::plic::handle_external_interrupt();
            }
            _ => {
                panic!("[suba] unexpected interrupt: scause={:#x}", scause);
            }
        }
    } else {
        // ---- 异常处理 ----
        match scause {
            0 => {
                let stval = read_stval();
                panic!("[suba] instruction misaligned: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            2 => {
                let stval = read_stval();
                panic!("[suba] illegal instruction: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            5 => {
                let stval = read_stval();
                panic!("[suba] load misaligned: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            7 => {
                let stval = read_stval();
                panic!("[suba] store misaligned: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            8 => {
                // Environment call from U-mode（用户态系统调用）
                tf.sepc = tf.sepc.wrapping_add(4);
                // dispatch_syscall(tf);  // 后续 feature
            }
            12 => {
                let stval = read_stval();
                panic!("[suba] instruction page fault: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            13 => {
                let stval = read_stval();
                panic!("[suba] load page fault: sepc={:#x}, stval={:#x}", tf.sepc, stval);
            }
            15 => {
                let stval = read_stval();
                panic!("[suba] store page fault: sepc={:#x}, stval={:#x}", tf.sepc, stval);
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

    // trap_return: 恢复寄存器，执行 sret
    // SAFETY: trap_frame 指向有效的 TrapFrame
    unsafe {
        trap_return(trap_frame);
    }
}
