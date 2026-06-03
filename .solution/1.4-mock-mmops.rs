/// Mock 内存管理实现。
pub struct MockMmOps;

impl MmOps for MockMmOps {
    unsafe fn translate_va(va: VA) -> Option<PA> {
        // Mock: 直接映射，虚拟地址 == 物理地址
        Some(PA::new(va.as_usize()))
    }

    fn flush_tlb() {
        // Mock: 无操作
    }

    fn flush_tlb_addr(_addr: usize) {
        // Mock: 无操作
    }

    fn current_page_table() -> PA {
        PA::new(0)
    }

    unsafe fn switch_page_table(_pt_root: PA) {
        // Mock: 无操作
    }
}
