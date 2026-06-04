// .solution/8.5-kernel-page-table.rs — 内核页表初始化参考实现
//
// 本文件是 feature 8.5 的完整参考实现。
// 学生应在 os/src/arch/riscv64/mod.rs 和 os/src/main.rs 中实现。

// ============================================================================
// 内核页表 — 身份映射 (Identity Mapping)
// ============================================================================

/// 内核页表使用身份映射：虚拟地址 = 物理地址
///
/// 优点：简单，不需要修改链接脚本或使用虚拟地址
/// 缺点：无法区分内核/用户空间的权限
///
/// 使用 1GB 大页 (superpage) 只需 4 个 PTE 就能映射 4GB。

// 根页表（4KB 对齐）
#[repr(C, align(4096))]
struct KernelPageTable {
    entries: [PageTableEntry; 512],
}

static mut KERNEL_PAGE_TABLE: KernelPageTable = KernelPageTable {
    entries: [PageTableEntry::empty(); 512],
};

/// 初始化内核页表
pub fn init_kernel_page_table() {
    // 1GB 大页标志：V|R|W|X|A|D|G
    let flags = PteFlags(
        PteFlags::VALID | PteFlags::READ | PteFlags::WRITE |
        PteFlags::EXECUTE | PteFlags::ACCESSED | PteFlags::DIRTY | PteFlags::GLOBAL
    );

    // 映射 4 个 1GB 区域，覆盖完整 4GB 物理地址空间
    unsafe {
        for i in 0..4 {
            let pa_base = i * (1 << 30); // 1GB
            let ppn = pa_base >> 12;
            KERNEL_PAGE_TABLE.entries[i] = PageTableEntry::new_leaf(ppn as u64, flags);
        }
    }

    // 激活 SV39 分页
    let root_ppn = (&raw const KERNEL_PAGE_TABLE as *const _ as usize) >> 12;
    let satp_val: usize = (8_usize << 60) | root_ppn;
    unsafe {
        asm!("csrw satp, {satp}", "sfence.vma", satp = in(reg) satp_val);
    }
}

// ============================================================================
// satp 操作
// ============================================================================

/// 读取 satp 寄存器
pub fn read_satp() -> usize {
    let satp: usize;
    unsafe { asm!("csrr {}, satp", out(reg) satp) };
    satp
}

/// 切换页表
pub unsafe fn switch_page_table(root_ppn: usize) {
    let satp_val = (8_usize << 60) | root_ppn;
    unsafe { asm!("csrw satp, {0}", "sfence.vma", in(reg) satp_val) };
}

// ============================================================================
// TLB 操作
// ============================================================================

/// 刷新全部 TLB
pub fn flush_tlb_all() {
    unsafe { asm!("sfence.vma") };
}

/// 刷新特定地址的 TLB
pub fn flush_tlb_addr(addr: usize) {
    unsafe { asm!("sfence.vma {0}, zero", in(reg) addr) };
}
