// .solution/8.8-riscv-mmops.rs — RISC-V MmOps 实现参考
//
// 本文件是 feature 8.8 的参考实现。
// 实际代码位于 os/src/arch/riscv64/mod.rs 的 Riscv64MmOps 结构体。
//
// ## MmOps trait 方法实现
//
// 1. translate_va(va: VA) -> Option<PA>
//    - 读取 satp 获取根页表 PPN
//    - 调用 page::translate_va() 进行三级页表遍历
//    - 返回翻译后的物理地址
//
// 2. flush_tlb()
//    - 执行 sfence.vma 指令
//    - 清除所有 TLB 缓存
//
// 3. flush_tlb_addr(addr: usize)
//    - 执行 sfence.vma vaddr, zero
//    - 仅清除特定地址的 TLB 缓存
//
// 4. current_page_table() -> PA
//    - 读取 satp 寄存器
//    - 提取 PPN 字段（低 44 位）
//    - 转换为物理地址（PPN << 12）
//
// 5. switch_page_table(pt_root: PA)
//    - 计算 PPN = pt_root >> 12
//    - 写入 satp：MODE=SV39(8) | PPN
//    - 执行 sfence.vma 刷新 TLB
//
// === 关键代码片段 ===

// pub struct Riscv64MmOps;
//
// impl MmOps for Riscv64MmOps {
//     unsafe fn translate_va(va: VA) -> Option<PA> {
//         let satp = read_satp();
//         let root_ppn = satp & 0x0FFF_FFFF_FFFF;
//         match page::translate_va(va.as_usize(), root_ppn, page::phys_read_u64) {
//             Ok(pa) => Some(PA::new(pa)),
//             Err(_) => None,
//         }
//     }
//
//     fn flush_tlb() {
//         flush_tlb_all();
//     }
//
//     fn flush_tlb_addr(addr: usize) {
//         flush_tlb_addr(addr);
//     }
//
//     fn current_page_table() -> PA {
//         let satp = read_satp();
//         let root_ppn = satp & 0x0FFF_FFFF_FFFF;
//         PA::new(root_ppn << 12)
//     }
//
//     unsafe fn switch_page_table(pt_root: PA) {
//         let root_ppn = pt_root.as_usize() >> 12;
//         unsafe { switch_page_table(root_ppn); }
//     }
// }

// === 教学要点 ===
//
// 1. MmOps 是内核与 MMU 硬件之间的桥梁
//    - 内核代码通过 MmOps trait 访问页表功能
//    - 不同架构实现不同的 MmOps
//
// 2. satp 寄存器是页表的"根指针"
//    - satp.PPN 指向根页表的物理地址
//    - 切换进程 = 切换 satp
//
// 3. TLB 是页表的缓存
//    - 修改页表后必须刷新 TLB
//    - sfence.vma 是 RISC-V 的 TLB 刷新指令
//
// 4. translate_va 使用三级页表遍历
//    - 从根页表开始，按 VPN[2]→VPN[1]→VPN[0] 逐级查找
//    - 最终找到叶子 PTE，计算物理地址
