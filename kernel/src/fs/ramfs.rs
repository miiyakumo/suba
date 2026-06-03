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

impl VfsFile for RamFs {
    fn read(&mut self, _buf: &mut [u8]) -> Result<usize, ()> {
        // TODO: 学生实现
        // 1. 从 self.offset 开始读取
        // 2. 读取 min(buf.len(), 剩余数据量) 字节
        // 3. 更新 offset
        // 4. 返回实际读取的字节数
        todo!("实现 RamFs::read")
    }

    fn write(&mut self, _buf: &[u8]) -> Result<usize, ()> {
        // TODO: 学生实现
        // 1. 如果 offset 超出当前长度，扩展 data
        // 2. 将 buf 写入 offset 位置
        // 3. 更新 offset
        // 4. 返回写入的字节数
        todo!("实现 RamFs::write")
    }

    fn size(&self) -> usize {
        self.data.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn ramfs_creation() {
        let fs = RamFs::new();
        assert_eq!(fs.size(), 0);
        assert_eq!(fs.offset(), 0);
    }

    #[test]
    fn ramfs_with_data() {
        let fs = RamFs::with_data(vec![1, 2, 3, 4, 5]);
        assert_eq!(fs.size(), 5);
    }

    #[test]
    fn ramfs_seek() {
        let mut fs = RamFs::new();
        fs.seek(42);
        assert_eq!(fs.offset(), 42);
    }
}
