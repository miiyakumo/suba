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
}
