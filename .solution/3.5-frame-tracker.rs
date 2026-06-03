/// 帧追踪器（RAII 自动回收）。
///
/// 拥有一个物理帧的所有权，Drop 时自动释放。
/// 这是 Rust RAII 模式在 OS 中的典型应用。
pub struct FrameTracker {
    frame: Frame,
}

impl FrameTracker {
    /// 创建帧追踪器
    pub fn new(frame: Frame) -> Self {
        Self { frame }
    }

    /// 获取帧引用
    pub fn frame(&self) -> &Frame {
        &self.frame
    }

    /// 获取物理页号
    pub fn ppn(&self) -> usize {
        self.frame.ppn
    }
}
