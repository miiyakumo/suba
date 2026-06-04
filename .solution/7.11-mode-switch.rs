// .solution/7.11-mode-switch.rs — S/U Mode 切换参考实现
//
// 本文件是 feature 7.11 的完整参考实现。
// 学生应在 os/src/arch/riscv64/mod.rs 中的 TODO(student) 处自行实现。

/// 从 S-mode 切换到 U-mode 并跳转到用户入口
///
/// ## 教学概念：RISC-V 特权级切换
///
/// RISC-V 的特权级切换通过 `sret` 指令完成：
/// 1. `sret` 将 `sstatus.SPP` 的值作为目标特权级（0=User, 1=Supervisor）
/// 2. `sret` 将 `sepc` 的值写回 PC（跳转到用户入口）
/// 3. `sret` 将 `sstatus.SPIE` 的值恢复到 `sstatus.SIE`（启用中断）
///
/// 因此，切换到 U-mode 需要：
/// - sstatus.SPP = 0 (User)
/// - sstatus.SPIE = 1 (sret 后启用中断)
/// - sstatus.SIE = 0 (当前关闭中断)
/// - sepc = 用户入口地址
/// - x2_sp = 用户栈指针
/// - kernel_sp = 内核栈指针（从 U-mode 陷阱回来时使用）
///
/// # Safety
///
/// - `user_entry` 必须是有效的用户态代码地址
/// - `user_sp` 必须是有效的用户栈顶地址
/// - `kernel_sp` 必须是有效的内核栈顶地址
pub unsafe fn drop_to_user_mode(user_entry: usize, user_sp: usize, kernel_sp: usize) {
    // 创建用户态陷阱帧
    let mut tf = TrapFrame::new();

    // 设置 sepc = 用户入口地址
    tf.sepc = user_entry;

    // 设置 sstatus
    // SPP (bit 8) = 0: sret 返回到 U-mode
    // SPIE (bit 5) = 1: sret 后恢复中断使能
    // SIE (bit 1) = 0: 当前中断关闭
    tf.sstatus = 1 << 5; // SPIE=1, SPP=0, SIE=0

    // 设置用户栈指针
    tf.x2_sp = user_sp;

    // 设置内核栈指针（从 U-mode 陷入时 trap_entry 会用到）
    tf.kernel_sp = kernel_sp;

    // 通过 trap_return 切换到用户态
    // SAFETY: tf 是刚刚创建的有效 TrapFrame
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }
}

/// 配置用户态陷阱帧（不立即切换）
///
/// 适用于需要在切换前做更多准备工作的场景。
pub unsafe fn setup_user_trap_frame(
    tf: &mut TrapFrame,
    user_entry: usize,
    user_sp: usize,
    kernel_sp: usize,
) {
    tf.sepc = user_entry;
    // SPP=0 (User), SPIE=1, SIE=0
    tf.sstatus = 1 << 5;
    tf.x2_sp = user_sp;
    tf.kernel_sp = kernel_sp;
}
