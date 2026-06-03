/// 位图帧分配器。
///
/// 使用位图跟踪物理帧的分配状态。每个 bit 代表一个帧：
/// - 0 = 空闲
/// - 1 = 已分配
///
/// ## 教学概念
/// 位图分配器是最简单的物理帧分配器之一。
/// 优点：实现简单、内存占用可预测。
/// 缺点：分配时需要扫描位图，O(n) 复杂度。
pub struct BitmapFrameAllocator {
    /// 位图，每个 u64 管理 64 个帧
    bitmap: alloc::vec::Vec<u64>,
    /// 管理的起始物理页号
    start_ppn: usize,
    /// 管理的总帧数
    total_frames: usize,
}

impl BitmapFrameAllocator {
    /// 创建新的位图帧分配器
    pub fn new(start_ppn: usize, total_frames: usize) -> Self {
        let words = (total_frames + 63) / 64;
        Self {
            bitmap: alloc::vec![0u64; words],
            start_ppn,
            total_frames,
        }
    }

    /// 标记指定帧为已分配
    fn set_allocated(&mut self, index: usize) {
        let word = index / 64;
        let bit = index % 64;
        self.bitmap[word] |= 1u64 << bit;
    }

    /// 检查指定帧是否已分配
    fn is_allocated(&self, index: usize) -> bool {
        let word = index / 64;
        let bit = index % 64;
        (self.bitmap[word] & (1u64 << bit)) != 0
    }
}

impl FrameAllocator for BitmapFrameAllocator {
    fn allocate(&mut self) -> Option<Frame> {
        for i in 0..self.total_frames {
            if !self.is_allocated(i) {
                self.set_allocated(i);
                return Some(Frame::new(self.start_ppn + i));
            }
        }
        None
    }

    unsafe fn deallocate(&mut self, frame: Frame) {
        // SAFETY: 调用者确保 frame 是通过 allocate 分配的
        let index = frame.ppn - self.start_ppn;
        if index < self.total_frames {
            let word = index / 64;
            let bit = index % 64;
            self.bitmap[word] &= !(1u64 << bit);
        }
    }
}
