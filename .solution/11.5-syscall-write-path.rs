// Solution: 11.5 - 用户态 syscall write 路径
//
// 本文件展示完整的 syscall write 路径。
// 完整代码位于 os/src/arch/riscv64/mod.rs。
//
// ============================================================================
// 完整 syscall 路径
// ============================================================================
//
// 用户程序:
//   li a7, 64    # SYS_WRITE = 64
//   li a0, 1     # fd = 1 (stdout)
//   la a1, buf   # 用户缓冲区地址
//   li a2, 5     # len = 5
//   ecall        # 触发系统调用
//
// 内核处理流程:
//   1. ecall 触发异常 → CPU 跳转到 stvec (trap_entry)
//   2. trap_entry 保存全部寄存器到 TrapFrame
//   3. trap_handler 读取 scause == 8 (U-mode ecall)
//   4. sepc += 4 (跳过 ecall 指令)
//   5. 调用 suba_kernel::syscall::dispatch(tf)
//   6. dispatch 读取 a7 == 64, 调用 sys_write(a0, a1, a2)
//   7. sys_write 通过 FdTable 查找 fd=1 (stdout)
//   8. VfsFile::write() 将数据写入 UART
//   9. 返回值写入 a0 (写入字节数)
//   10. trap_return 恢复寄存器, sret 返回用户态
//
// ============================================================================
// trap_handler 中的关键代码
// ============================================================================
//
// match scause {
//     8 => {
//         // U-mode ecall: 系统调用
//         tf.sepc = tf.sepc.wrapping_add(4);  // 跳过 ecall 指令
//         suba_kernel::syscall::dispatch(tf);  // 分发到对应处理函数
//     }
//     ...
// }
//
// ============================================================================
// sepc += 4 的重要性
// ============================================================================
//
// ecall 指令触发异常时，sepc 指向 ecall 本身。
// 如果不加 4，sret 后会重新执行 ecall，导致无限循环。
//
// 这与 x86 的 syscall 不同：x86 的 RCX 保存返回地址（下一条指令），
// 而 RISC-V 的 sepc 保存的是触发异常的指令本身。
//
// ============================================================================
// dispatch 函数
// ============================================================================
//
// pub fn dispatch<F: SyscallFrame>(frame: &mut F) {
//     let ret = match frame.syscall_id() {   // 读取 a7
//         SYS_WRITE => sys_write(frame.arg0(), frame.arg1(), frame.arg2()),
//         SYS_READ  => sys_read(frame.arg0(), frame.arg1(), frame.arg2()),
//         SYS_EXIT  => sys_exit(frame.arg0() as i32),
//         ...
//         _ => (-1isize) as usize,
//     };
//     frame.set_ret(ret);  // 写入 a0
// }
