/// 帧分配器 trait。
///
/// 管理空闲物理帧的分配和释放。
pub trait FrameAllocator {
    /// 分配一个物理帧
    fn allocate(&mut self) -> Option<Frame>;

    /// 释放一个物理帧
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `frame` 是之前通过 `allocate` 分配的。
    unsafe fn deallocate(&mut self, frame: Frame);
}
