// Solution: 10.7 - 初始化用户态 TrapFrame
//
// 本文件展示如何设置用户态的初始陷阱帧，包括 a0 = argc。
// 完整代码位于 os/src/arch/riscv64/mod.rs。

// ============================================================================
// TrapFrame::set_user_trap_frame — 设置用户态初始陷阱帧
// ============================================================================

impl TrapFrame {
    /// 设置用户态的初始陷阱帧
    ///
    /// 用于从 S-mode 切换到 U-mode：设置 sstatus.SPP = User，
    /// sepc = 用户入口地址，SPIE = 1（sret 后启用中断），
    /// a0 = argc（用户程序参数）。
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
    /// ## a0 = argc 的 ABI 约定
    ///
    /// RISC-V 调用约定中，a0（x10）既是第一个参数寄存器，也是返回值寄存器。
    /// 用户程序的 `_start` 入口点通过 a0 获取 argc（参数个数）。
    /// 对于简单的单程序内核，argc 通常为 0 或 1。
    ///
    /// # 参数
    /// - `entry`: 用户程序入口地址（sret 后的 PC）
    /// - `user_sp`: 用户栈顶地址
    /// - `kernel_sp`: 内核栈顶地址（从用户态 trap 回来时使用）
    /// - `argc`: 传递给用户程序的参数（写入 a0 寄存器）
    pub fn set_user_trap_frame(&mut self, entry: usize, user_sp: usize, kernel_sp: usize, argc: usize) {
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
        self.x10_a0 = argc; // 用户程序通过 a0 获取 argc
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
// enter_user_mode — 从 S-mode 切换到 U-mode（带 argc）
// ============================================================================

pub fn enter_user_mode(entry: usize, user_sp: usize, kernel_sp: usize, argc: usize) -> ! {
    let mut tf = TrapFrame::new();
    tf.set_user_trap_frame(entry, user_sp, kernel_sp, argc);

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
    // SAFETY: tf 是正确初始化的 TrapFrame
    unsafe {
        trap_return(&mut tf as *mut TrapFrame);
    }

    unreachable!()
}

// ============================================================================
// 使用方式
// ============================================================================

// 1. 加载 ELF 并获取入口点和栈顶：
//    let (entry, stack_top) = Riscv64ElfLoader::load_elf(elf_data)?;
//
// 2. 创建 TrapFrame 并设置用户态初始状态（argc = 0）：
//    let mut tf = TrapFrame::new();
//    tf.set_user_trap_frame(entry, stack_top, kernel_sp, 0);
//
// 3. 切换页表并 sret 到用户态：
//    unsafe { user_space.activate(); }
//    unsafe { trap_return(&mut tf as *mut TrapFrame); }
//
// 或直接使用 enter_user_mode：
//    unsafe { user_space.activate(); }
//    enter_user_mode(entry, stack_top, kernel_sp, 0);
