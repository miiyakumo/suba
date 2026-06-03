/// 页表 trait。
///
/// 提供虚拟地址到物理地址的映射操作。
pub trait PageTable {
    /// 映射一个虚拟页到物理帧
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址对齐且不冲突。
    unsafe fn map(&mut self, va: usize, pa: usize, flags: crate::arch::PteFlags) -> Result<(), ()>;

    /// 取消映射
    fn unmap(&mut self, va: usize) -> Result<(), ()>;

    /// 查询映射
    fn translate(&self, va: usize) -> Option<usize>;
}
