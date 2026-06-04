// =============================================================================
// Solution: 10.8 — 实现 RISC-V Arch trait 完整实现
// =============================================================================
//
// 本文件展示如何为 RISC-V 实现 Arch trait，整合所有后端组件。
//
// ## 核心概念
//
// Arch trait 是内核与硬件之间的最高层抽象，组合了：
// 1. CpuOps: CPU 操作（中断管理、halt）
// 2. MmOps: 内存管理（页表操作）
// 3. context_switch: 上下文切换
// 4. copy_from_user/to_user: 用户/内核内存复制
//
// ## 实现要点
//
// ### 1. Riscv64Arch 结构体
//
// ```rust
// pub struct Riscv64Arch;
// ```
//
// 这是一个零大小的标记类型，用于实现 trait。
//
// ### 2. CpuOps 实现
//
// 委托给 Riscv64CpuOps：
// - id(): 读取 mhartid
// - halt(): wfi 指令
// - enable/disable_interrupts(): 操作 sstatus.SIE
//
// ### 3. MmOps 实现
//
// 委托给 Riscv64MmOps：
// - translate_va(): SV39 三级页表翻译
// - flush_tlb(): sfence.vma
// - switch_page_table(): 写入 satp
//
// ### 4. context_switch 实现
//
// 调用 switch.S 中的汇编代码：
//
// ```rust
// unsafe fn context_switch(old: *mut Context, new: *const Context) {
//     unsafe { switch(old, new); }
// }
// ```
//
// switch.S 保存 callee-saved 寄存器（ra, sp, s0-s11）到 old，
// 从 new 恢复寄存器并跳转到 new.ra。
//
// ### 5. copy_from_user 实现
//
// 通过页表翻译用户虚拟地址：
//
// ```rust
// unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
//     // 逐页处理
//     while remaining > 0 {
//         // 翻译用户 VA → 物理地址
//         let user_pa = translate_user_va(user_va)?;
//         // 通过内核身份映射复制
//         unsafe { core::ptr::copy_nonoverlapping(user_pa as *const u8, kernel_dst, copy_len); }
//     }
// }
// ```
//
// ### 6. copy_to_user 实现
//
// 与 copy_from_user 对称：
//
// ```rust
// unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
//     // 逐页处理
//     while remaining > 0 {
//         // 翻译用户 VA → 物理地址
//         let user_pa = translate_user_va(user_va)?;
//         // 通过内核身份映射复制
//         unsafe { core::ptr::copy_nonoverlapping(kernel_src, user_pa as *mut u8, copy_len); }
//     }
// }
// ```
//
// ## 教学概念：用户/内核内存隔离
//
// 用户态和内核态使用不同的页表。内核不能直接解引用用户虚拟地址，
// 必须先通过页表翻译为物理地址，再通过内核的身份映射访问。
//
// ```text
// 用户 VA → [页表翻译] → 物理地址 → [内核身份映射] → 内核访问
// ```
//
// ## 参考实现
//
// comix 使用宏 impl_arch! 生成 Arch trait 实现：
//
// ```rust
// impl_arch!(
//     cpu_ops::Riscv64,
//     memory::Riscv64ProcessAddressSpace,
//     memory::Riscv64KernelAddressSpace
// );
// ```
//
// suba 采用手动实现，更清晰地展示每个方法的实现细节。
//
// ## 关键点
//
// 1. **context_switch vs trap_return**:
//    - context_switch: 自愿切换，只保存 callee-saved 寄存器
//    - trap_return: 强制切换，保存全部寄存器（TrapFrame）
//
// 2. **逐页复制**: copy_from_user/to_user 需要逐页处理，
//    因为用户虚拟地址可能跨越多个物理页
//
// 3. **translate_user_va**: 使用当前 satp 中的页表进行翻译
//
// 4. **错误处理**: 翻译失败返回 Err(())，调用者处理错误
