// .solution/7.11-mode-switch.rs — S/U Mode 切换参考实现
//
// 本文件是 feature 7.11 的完整参考实现。
// 学生应在 os/src/arch/riscv64/mod.rs 中的 TODO(student) 处自行实现。

// ============================================================================
// 方法一：TrapFrame 方法（推荐）
// ============================================================================

impl TrapFrame {
    /// 设置用户态的初始陷阱帧
    ///
    /// 用于从 S-mode 切换到 U-mode：设置 sstatus.SPP = User，
    /// sepc = 用户入口地址，SPIE = 1（sret 后启用中断）。
    ///
    /// ## 教学概念：S/U Mode 切换
    ///
    /// RISC-V 的特权级切换通过 `sret` 指令完成：
    /// 1. `sstatus.SPP` 决定 sret 后的特权级：
    ///    - SPP = 0 (User): sret 后进入 U-mode
    ///    - SPP = 1 (Supervisor): sret 后留在 S-mode
    /// 2. `sepc` 决定 sret 后的 PC（从哪里开始执行）
    /// 3. `sstatus.SPIE` 决定 sret 后的中断状态：
    ///    - sret 自动将 SPIE 恢复到 SIE 位
    ///    - 所以 SPIE=1 意味着 sret 后中断启用
    ///
    /// ## sstatus 位域
    ///
    /// ```text
    /// bit 8 (SPP):  0 = User, 1 = Supervisor
    /// bit 5 (SPIE): 1 = sret 后启用中断
    /// bit 1 (SIE):  0 = 当前中断关闭
    /// ```
    ///
    /// # 参数
    /// - `entry`: 用户程序入口地址（sret 后的 PC）
    /// - `user_sp`: 用户栈顶地址
    /// - `kernel_sp`: 内核栈顶地址（从用户态 trap 回来时使用）
    pub fn set_user_trap_frame(&mut self, entry: usize, user_sp: usize, kernel_sp: usize) {
        // sstatus 设置：
        // - SPP (bit 8) = 0: sret 后进入 U-mode
        // - SPIE (bit 5) = 1: sret 后 SIE = 1（启用中断）
        // - SIE (bit 1) = 0: 当前中断关闭
        let sstatus: usize = 1 << 5; // SPIE=1, SPP=0, SIE=0
        self.sepc = entry;
        self.sstatus = sstatus;
        self.x2_sp = user_sp;
        self.kernel_sp = kernel_sp;
        // 清零其他寄存器（用户程序从干净状态开始）
        self.x1_ra = 0;
        self.x3_gp = 0;
        self.x4_tp = 0;
        self.x5_t0 = 0;
        self.x6_t1 = 0;
        self.x7_t2 = 0;
        self.x8_s0 = 0;
        self.x9_s1 = 0;
        self.x10_a0 = 0;
        self.x11_a1 = 0;
        self.x12_a2 = 0;
        self.x13_a3 = 0;
        self.x14_a4 = 0;
        self.x15_a5 = 0;
        self.x16_a6 = 0;
        self.x17_a7 = 0;
        self.x18_s2 = 0;
        self.x19_s3 = 0;
        self.x20_s4 = 0;
        self.x21_s5 = 0;
        self.x22_s6 = 0;
        self.x23_s7 = 0;
        self.x24_s8 = 0;
        self.x25_s9 = 0;
        self.x26_s10 = 0;
        self.x27_s11 = 0;
        self.x28_t3 = 0;
        self.x29_t4 = 0;
        self.x30_t5 = 0;
        self.x31_t6 = 0;
    }
}

// ============================================================================
// 方法二：enter_user_mode 函数 — 完整的模式切换
// ============================================================================

/// 从 S-mode 切换到 U-mode
///
/// 设置用户态陷阱帧后，通过 `trap_return` 执行 `sret` 切换到用户态。
///
/// ## 教学概念：特权级切换的完整流程
///
/// ```text
/// S-mode (内核)                          U-mode (用户)
/// ┌─────────────────┐                   ┌─────────────────┐
/// │ 1. 准备 TrapFrame│                   │ 用户程序从       │
/// │    SPP=User      │                   │ entry 开始执行   │
/// │    sepc=entry    │                   │                  │
/// │    SPIE=1        │                   │                  │
/// │                  │                   │                  │
/// │ 2. 写 sscratch   │                   │                  │
/// │    (TrapFrame指针)│                   │                  │
/// │                  │                   │                  │
/// │ 3. trap_return() │  ─── sret ───►    │                  │
/// │    恢复寄存器     │   SPP=0→U-mode    │                  │
/// │    sret          │   sepc→entry      │                  │
/// └─────────────────┘   SPIE→SIE=1      └─────────────────┘
/// ```
///
/// # 参数
/// - `entry`: 用户程序入口地址
/// - `user_sp`: 用户栈顶地址
/// - `kernel_sp`: 内核栈顶地址（从用户态 trap 回来时切换到此栈）
pub fn enter_user_mode(entry: usize, user_sp: usize, kernel_sp: usize) -> ! {
    // 创建用户态陷阱帧
    let mut tf = TrapFrame::new();
    tf.set_user_trap_frame(entry, user_sp, kernel_sp);

    // 将 TrapFrame 指针写入 sscratch（供下次从用户态 trap 回来时使用）
    // SAFETY: tf 是栈上有效的 TrapFrame，生命周期覆盖整个函数
    unsafe {
        asm!(
            "csrw sscratch, {tf_ptr}",
            tf_ptr = in(reg) &tf as *const TrapFrame as usize,
            options(nomem, nostack)
        );
    }

    // 通过 trap_return 执行 sret，切换到用户态
    // trap_return 在 trap.S 中实现：恢复全部寄存器，执行 sret
    // sret 硬件行为：PC ← sepc, SIE ← SPIE, 特权级 ← SPP
    // SAFETY: tf 是正确初始化的 TrapFrame，包含有效的 sstatus 和 sepc
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }

    // trap_return 不应返回（已切换到用户态）
    unreachable!()
}

// ============================================================================
// 方法三：独立函数（不依赖方法语法）
// ============================================================================

/// 配置用户态陷阱帧（不立即切换）
///
/// 适用于需要在切换前做更多准备工作的场景。
pub fn setup_user_trap_frame(
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

/// 从 S-mode 切换到 U-mode（简化版）
///
/// 通过 trap_return 执行 sret 切换到用户态。
///
/// # Safety
///
/// - `user_entry` 必须是有效的用户态代码地址
/// - `user_sp` 必须是有效的用户栈顶地址
/// - `kernel_sp` 必须是有效的内核栈顶地址
pub unsafe fn drop_to_user_mode(user_entry: usize, user_sp: usize, kernel_sp: usize) {
    let mut tf = TrapFrame::new();
    setup_user_trap_frame(&mut tf, user_entry, user_sp, kernel_sp);

    // SAFETY: tf 是刚刚创建的有效 TrapFrame
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }
}
