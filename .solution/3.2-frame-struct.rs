/// 物理帧
///
/// 代表一个 4KB 物理内存帧，由物理页号 (PPN) 标识。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    /// 物理页号
    pub ppn: usize,
}

impl Frame {
    /// 从物理页号创建帧
    pub fn new(ppn: usize) -> Self {
        Self { ppn }
    }

    /// 从物理地址创建帧
    pub fn from_pa(pa: usize) -> Self {
        Self {
            ppn: pa >> PAGE_SHIFT,
        }
    }

    /// 获取帧的物理地址
    pub fn start_pa(&self) -> usize {
        self.ppn << PAGE_SHIFT
    }
}
