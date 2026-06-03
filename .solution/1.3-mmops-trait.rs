/// 内存管理操作 trait。
///
/// 提供地址空间相关的架构操作，如地址转换、TLB 管理等。
pub trait MmOps: 'static {
    /// 将虚拟地址翻译为物理地址
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址有效。
    unsafe fn translate_va(va: VA) -> Option<PA>;

    /// 刷新 TLB
    fn flush_tlb();

    /// 刷新指定地址的 TLB 条目
    fn flush_tlb_addr(addr: usize);

    /// 获取当前页表根物理地址
    fn current_page_table() -> PA;

    /// 切换页表
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `pt_root` 指向有效的页表。
    unsafe fn switch_page_table(pt_root: PA);
}
