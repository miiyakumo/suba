// Solution: 11.4 - sret 切换到用户态
//
// 本文件展示如何通过 sret 指令从 S-mode 切换到 U-mode。
// 完整代码位于 os/src/main.rs 和 os/src/arch/riscv64/mod.rs。
//
// ============================================================================
// sret 切换流程
// ============================================================================
//
// 1. 加载 init ELF 程序到用户地址空间
//    let init_elf = include_bytes!("user_init.bin");
//    let (user_space, entry, stack_top) = load_elf_to_space(init_elf)?;
//
// 2. 激活用户页表
//    unsafe { user_space.activate(); }
//
// 3. 调用 enter_user_mode() 切换到用户态
//    enter_user_mode(entry, stack_top, kernel_sp, 0);
//
// ============================================================================
// enter_user_mode 内部实现
// ============================================================================
//
// pub fn enter_user_mode(entry, user_sp, kernel_sp, argc) -> ! {
//     // 1. 创建 TrapFrame
//     let mut tf = TrapFrame::new();
//
//     // 2. 设置 sepc = 用户入口地址
//     //    sret 会将 sepc 写回 PC
//     tf.sepc = entry;
//
//     // 3. 设置 sstatus
//     //    SPP (bit 8) = 0: sret 后进入 U-mode
//     //    SPIE (bit 5) = 1: sret 后启用中断
//     //    SIE (bit 1) = 0: 当前中断关闭
//     tf.sstatus = 1 << 5;
//
//     // 4. 设置用户栈指针
//     tf.x2_sp = user_sp;
//
//     // 5. 设置内核栈指针
//     //    从用户态 trap 回来时，trap_entry 使用此值切换到内核栈
//     tf.kernel_sp = kernel_sp;
//
//     // 6. 设置 argc (a0)
//     tf.x10_a0 = argc;
//
//     // 7. 写入 sscratch
//     //    从用户态陷入时，trap_entry 读取 sscratch 获取 TrapFrame 地址
//     csrw sscratch, &tf
//
//     // 8. 调用 trap_return (trap.S)
//     //    trap_return 恢复全部寄存器，执行 sret
//     trap_return(&mut tf);
//
//     // sret 硬件行为：
//     //   PC ← sepc (用户入口)
//     //   特权级 ← SPP = 0 (U-mode)
//     //   SIE ← SPIE = 1 (中断启用)
// }
//
// ============================================================================
// sstatus 位域详解
// ============================================================================
//
// sstatus (Supervisor Status Register):
//   bit 8 (SPP):  Previous Privilege Level
//     - 0: sret 返回到 U-mode
//     - 1: sret 返回到 S-mode
//
//   bit 5 (SPIE): Supervisor Interrupt Previous Enable
//     - sret 自动将 SPIE 恢复到 SIE
//     - SPIE=1 意味着 sret 后中断启用
//
//   bit 1 (SIE):  Supervisor Interrupt Enable
//     - 当前中断使能状态
//     - 切换到用户态前设为 0（由 SPIE 在 sret 后恢复）
//
// ============================================================================
// sscratch CSR 的作用
// ============================================================================
//
// sscratch 保存内核栈上的 TrapFrame 指针。
//
// 从 U-mode 陷入 S-mode 时：
//   1. CPU 跳转到 stvec（trap_entry 汇编代码）
//   2. trap_entry 读取 sscratch 获取 TrapFrame 地址
//   3. 交换 sp 和 sscratch（sp → TrapFrame, kernel_sp → sp）
//   4. 保存全部寄存器到 TrapFrame
//   5. 调用 trap_handler
//
// 返回 U-mode 时：
//   1. trap_return 从 TrapFrame 恢复全部寄存器
//   2. 交换 sp 和 sscratch（恢复用户 sp）
//   3. sret 切换到 U-mode
//
// ============================================================================
// 用户程序 (user_init.bin)
// ============================================================================
//
// 最小 RISC-V 用户程序（12 字节 ELF）：
//   li a7, 93    # exit 系统调用号
//   li a0, 0     # exit code = 0
//   ecall        # 触发系统调用
//
// 这个程序立即退出，用于验证 sret 切换和 trap 返回的正确性。
// 后续可以替换为更复杂的用户程序（如 shell）。
