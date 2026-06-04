//! # RamFs — 内存文件系统
//!
//! 实现最简单的文件系统——所有数据存储在内存中。
//!
//! ## 教学概念
//! - 内存文件系统是理解文件系统概念的最佳起点
//! - 数据以字节数组形式存储，没有磁盘 I/O
//! - 适合教学演示和内核初始化阶段的临时文件系统

use alloc::vec::Vec;

use super::VfsFile;

/// 内存文件系统。
///
/// 使用 `Vec<u8>` 存储文件内容，支持基本的读写操作。
pub struct RamFs {
    /// 文件内容
    data: Vec<u8>,
    /// 当前读写位置
    offset: usize,
}

impl RamFs {
    /// 创建新的空 RamFs
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            offset: 0,
        }
    }

    /// 创建带初始数据的 RamFs
    pub fn with_data(data: Vec<u8>) -> Self {
        Self { data, offset: 0 }
    }

    /// 获取当前偏移量
    pub fn offset(&self) -> usize {
        self.offset
    }

    /// 设置偏移量
    pub fn seek(&mut self, offset: usize) {
        self.offset = offset;
    }
}

impl Default for RamFs {
    fn default() -> Self {
        Self::new()
    }
}

// read / write 实见 6.5 / 6.6 的 solution 文件
