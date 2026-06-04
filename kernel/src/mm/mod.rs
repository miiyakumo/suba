//! # 内存管理 (mm)
//!
//! 本模块实现了物理内存分配和虚拟内存映射。
//!
//! ## 教学概念
//! - **物理帧分配**：将物理内存划分为 4KB 帧，按需分配
//! - **FrameTracker**：RAII 自动回收物理帧，防止内存泄漏
//! - **页表**：虚拟地址到物理地址的映射表（RISC-V 使用 SV39 三级页表）
//! - **内核堆**：动态内存分配（使用 talc 分配器）
//!
//! ## 设计参考
//! - FrameTracker 的 RAII 设计参考 Rust 的所有权系统
//! - 页表操作参考 comix 的 SV39 实现

pub mod address;
pub mod heap;

/// 页大小（4KB）
pub const PAGE_SIZE: usize = 4096;
/// 页内偏移位数
pub const PAGE_SHIFT: usize = 12;

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

// 注意：FrameTracker 的 Drop 实现需要全局帧分配器，
// 将在具体分配器实现中添加。

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

// ---------------------------------------------------------------------------
// BitmapFrameAllocator — 位图帧分配器
// ---------------------------------------------------------------------------

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
    ///
    /// # 参数
    /// - `start_ppn`: 起始物理页号
    /// - `total_frames`: 管理的总帧数
    pub fn new(start_ppn: usize, total_frames: usize) -> Self {
        let words = total_frames.div_ceil(64);
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

// ---------------------------------------------------------------------------
// MockPageTable — Mock 页表
// ---------------------------------------------------------------------------

/// Mock 页表实现。
///
/// 使用简单的 Vec 存储映射关系，在测试环境中模拟页表操作。
pub struct MockPageTable {
    /// 映射列表：(虚拟地址, 物理地址, 标志)
    mappings: alloc::vec::Vec<(usize, usize, crate::arch::PteFlags)>,
}

impl MockPageTable {
    /// 创建新的 Mock 页表
    pub fn new() -> Self {
        Self {
            mappings: alloc::vec::Vec::new(),
        }
    }
}

impl Default for MockPageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTable for MockPageTable {
    unsafe fn map(&mut self, va: usize, pa: usize, flags: crate::arch::PteFlags) -> Result<(), ()> {
        // 检查是否已存在映射
        if self.mappings.iter().any(|(v, _, _)| *v == va) {
            return Err(());
        }
        self.mappings.push((va, pa, flags));
        Ok(())
    }

    fn unmap(&mut self, va: usize) -> Result<(), ()> {
        let len_before = self.mappings.len();
        self.mappings.retain(|(v, _, _)| *v != va);
        if self.mappings.len() < len_before {
            Ok(())
        } else {
            Err(())
        }
    }

    fn translate(&self, va: usize) -> Option<usize> {
        self.mappings
            .iter()
            .find(|(v, _, _)| *v == va)
            .map(|(_, pa, _)| *pa)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_basic() {
        let f = Frame::new(1);
        assert_eq!(f.start_pa(), PAGE_SIZE);
        assert_eq!(f.ppn, 1);
    }

    #[test]
    fn frame_from_pa() {
        let f = Frame::from_pa(0x8020_1000);
        assert_eq!(f.ppn, 0x80201);
        assert_eq!(f.start_pa(), 0x8020_1000);
    }

    #[test]
    fn bitmap_allocator_basic() {
        let mut alloc = BitmapFrameAllocator::new(0, 128);
        let f1 = alloc.allocate().unwrap();
        assert_eq!(f1.ppn, 0);
        let f2 = alloc.allocate().unwrap();
        assert_eq!(f2.ppn, 1);
        // 释放后可以重新分配
        unsafe { alloc.deallocate(f1) };
        let f3 = alloc.allocate().unwrap();
        assert_eq!(f3.ppn, 0); // 复用释放的帧
    }

    #[test]
    fn bitmap_allocator_exhaustion() {
        let mut alloc = BitmapFrameAllocator::new(100, 2);
        let f1 = alloc.allocate().unwrap();
        assert_eq!(f1.ppn, 100);
        let f2 = alloc.allocate().unwrap();
        assert_eq!(f2.ppn, 101);
        assert!(alloc.allocate().is_none()); // 已耗尽
    }

    #[test]
    fn mock_page_table_map_translate() {
        let mut pt = MockPageTable::new();
        let flags = crate::arch::PteFlags::new(crate::arch::PteFlags::VALID | crate::arch::PteFlags::READ);
        unsafe { pt.map(0x1000, 0x8000_1000, flags).unwrap() };
        assert_eq!(pt.translate(0x1000), Some(0x8000_1000));
        assert_eq!(pt.translate(0x2000), None);
    }

    #[test]
    fn mock_page_table_unmap() {
        let mut pt = MockPageTable::new();
        let flags = crate::arch::PteFlags::new(crate::arch::PteFlags::VALID);
        unsafe { pt.map(0x1000, 0x8000_1000, flags).unwrap() };
        pt.unmap(0x1000).unwrap();
        assert_eq!(pt.translate(0x1000), None);
    }

    #[test]
    fn mock_page_table_duplicate_map_fails() {
        let mut pt = MockPageTable::new();
        let flags = crate::arch::PteFlags::new(crate::arch::PteFlags::VALID);
        unsafe { pt.map(0x1000, 0x8000_1000, flags).unwrap() };
        assert!(unsafe { pt.map(0x1000, 0x8000_2000, flags) }.is_err());
    }
}
