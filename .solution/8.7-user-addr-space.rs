// .solution/8.7-user-addr-space.rs — 用户地址空间创建参考实现
//
// 本文件是 feature 8.7 的参考实现。
// 实际代码位于 os/src/arch/riscv64/page.rs 的 UserAddrSpace 结构体。
//
// ## 设计概述
//
// 用户地址空间 = 独立的 SV39 页表，包含：
// 1. 内核空间映射（从内核页表复制）
// 2. 用户代码/数据段映射（U 标志）
// 3. 用户栈映射
// 4. Trampoline 页映射
//
// ## 关键设计决策
//
// ### 为什么复制全部 512 个 PTE？
//
// suba 内核使用身份映射（VA = PA），内核代码在低地址 0x80200000。
// SV39 根页表的 PTE[0..4] 映射了 0x0000_0000 ~ 0xFFFF_FFFF（4GB）。
//
// 如果只复制高地址 PTE（索引 256..512），用户态 trap 进入内核后
// （satp 切换到用户页表），内核代码的低地址映射不存在，会触发页错误。
//
// 因此必须复制所有 512 个 PTE，确保内核的完整映射在用户页表中可用。
//
// ### Trampoline 页
//
// Trampoline 页映射在用户地址空间的最顶部（USER_TOP - PAGE_SIZE）。
// 它包含 trap entry/return 代码，在用户态和内核态之间共享同一虚拟地址。
//
// 当 CPU 从 U-mode 陷入 S-mode 时：
// 1. 硬件跳转到 stvec（trap_entry 地址）
// 2. 此时 satp 仍指向用户页表
// 3. trap_entry 代码必须在用户页表中也能访问
//
// 解决方案：将 trap entry 代码映射到用户地址空间顶部的 trampoline 页。
//
// ## 代码位置
//
// 实现位于 os/src/arch/riscv64/page.rs:
// - UserAddrSpace 结构体
// - new() — 创建用户地址空间
// - map_user_page() — 映射用户页（自动添加 U 标志）
// - map_kernel_page() — 映射内核页
// - map_segment() — 映射连续区域
// - map_user_stack() — 映射用户栈
// - map_trampoline() — 映射 trampoline 页
// - activate() — 激活页表
//
// === 关键代码片段 ===

// --- 1. UserAddrSpace::new() ---
//
// pub fn new() -> Result<Self, MapError> {
//     let root_ppn = alloc_zeroed_frame().ok_or(MapError::FrameAllocFailed)?;
//
//     // 从内核页表复制所有 PTE
//     let kernel_satp = read_satp_current();
//     let kernel_root_ppn = kernel_satp & 0x0FFF_FFFF_FFFF;
//
//     for i in 0..512 {
//         let src_addr = kernel_root_ppn * PAGE_SIZE + i * 8;
//         let dst_addr = root_ppn * PAGE_SIZE + i * 8;
//         if let Ok(pte_val) = phys_read_u64(src_addr) {
//             let _ = phys_write_u64(dst_addr, pte_val);
//         }
//     }
//
//     Ok(Self { root_ppn })
// }

// --- 2. map_user_page() ---
//
// pub unsafe fn map_user_page(&self, va: usize, pa: usize, flags: PteFlags)
//     -> Result<(), MapError>
// {
//     let user_flags = PteFlags(flags.0 | PteFlags::USER);
//     map_page(va, pa, user_flags, self.root_ppn,
//              phys_read_u64, phys_write_u64, alloc_zeroed_frame)
// }

// --- 3. map_user_stack() ---
//
// pub fn map_user_stack(&self) -> Result<usize, MapError> {
//     let num_pages = USER_STACK_SIZE / PAGE_SIZE;
//     let stack_bottom_va = USER_STACK_TOP - USER_STACK_SIZE;
//
//     for i in 0..num_pages {
//         let va = stack_bottom_va + i * PAGE_SIZE;
//         let pa_ppn = alloc_zeroed_frame().ok_or(MapError::FrameAllocFailed)?;
//         let pa = pa_ppn * PAGE_SIZE;
//         let flags = PteFlags(PteFlags::READ | PteFlags::WRITE);
//         unsafe { self.map_user_page(va, pa, flags)?; }
//     }
//     Ok(USER_STACK_TOP)
// }

// --- 4. map_trampoline() ---
//
// pub unsafe fn map_trampoline(&self, trap_entry_pa: usize) -> Result<(), MapError> {
//     let flags = PteFlags(PteFlags::VALID | PteFlags::READ | PteFlags::EXECUTE);
//     unsafe { self.map_kernel_page(TRAMPOLINE_VA, trap_entry_pa, flags)?; }
//     Ok(())
// }

// === 教学要点 ===
//
// 1. 用户地址空间 = 独立页表
//    - 每个进程有自己的 satp 值
//    - 切换进程 = 切换 satp
//
// 2. 内核映射共享
//    - 所有用户页表共享相同的内核映射
//    - 创建用户页表时从内核页表复制
//    - 陷入内核时无需切换页表
//
// 3. U 标志控制访问权限
//    - U=0: 仅 S-mode 可访问（内核页面）
//    - U=1: U-mode 和 S-mode 都可访问（用户页面）
//    - 硬件在 U-mode 访问 U=0 的页面时触发页错误
//
// 4. Trampoline 页
//    - 解决"trap 时 satp 还是用户页表"的问题
//    - 在用户页表和内核页表中映射到相同虚拟地址
//    - trap entry 代码在两个页表中都可访问
