// .solution/8.6-satp-switch.rs — satp 切换和 sfence.vma 参考实现
//
// 本文件是 feature 8.6 的参考实现。
// 这些函数已在 os/src/arch/riscv64/mod.rs 中随 8.5 一起实现。

/// 切换到新的页表
///
/// satp 格式：MODE[63:60] | ASID[59:44] | PPN[43:0]
/// - MODE = 8: SV39 模式
/// - ASID = 0: 地址空间 ID
/// - PPN: 根页表物理页号
///
/// # Safety
/// root_ppn 必须指向有效的 SV39 页表
pub unsafe fn switch_page_table(root_ppn: usize) {
    let satp_val: usize = (8_usize << 60) | root_ppn;
    unsafe {
        asm!(
            "csrw satp, {satp}",
            "sfence.vma",
            satp = in(reg) satp_val,
        );
    }
}

/// 刷新全部 TLB
pub fn flush_tlb_all() {
    unsafe { asm!("sfence.vma") };
}

/// 刷新特定虚拟地址的 TLB
pub fn flush_tlb_addr(addr: usize) {
    unsafe { asm!("sfence.vma {0}, zero", in(reg) addr) };
}

/// 读取当前 satp
pub fn read_satp() -> usize {
    let satp: usize;
    unsafe { asm!("csrr {}, satp", out(reg) satp) };
    satp
}
