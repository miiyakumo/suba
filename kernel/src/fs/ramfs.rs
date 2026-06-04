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
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
        // TODO: 学生实现
        // 1. 从 self.offset 开始读取
        // 2. 读取 min(buf.len(), 剩余数据量) 字节
        // 3. 更新 offset
        // 4. 返回实际读取的字节数
        if self.offset >= self.data.len() {
            return Ok(0);
        }
        let remaining = self.data.len() - self.offset;
        let to_read = buf.len().min(remaining);
        buf[..to_read].copy_from_slice(&self.data[self.offset..self.offset + to_read]);
        self.offset += to_read;
        Ok(to_read)
    }

    fn write(&mut self, buf: &[u8]) -> Result<usize, ()> {
        // TODO: 学生实现
        // 1. 如果 offset 超出当前长度，扩展 data
        // 2. 将 buf 写入 offset 位置
        // 3. 更新 offset
        // 4. 返回写入的字节数
        if buf.is_empty() {
            return Ok(0);
        }
        // 如果 offset 超出当前长度，用 0 填充
        if self.offset > self.data.len() {
            self.data.resize(self.offset, 0);
        }
        let end = self.offset + buf.len();
        if end > self.data.len() {
            self.data.resize(end, 0);
        }
        self.data[self.offset..end].copy_from_slice(buf);
        self.offset = end;
        Ok(buf.len())
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

    #[test]
    fn ramfs_read_basic() {
        let mut fs = RamFs::with_data(vec![10, 20, 30, 40, 50]);
        let mut buf = [0u8; 3];
        let n = fs.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(buf, [10, 20, 30]);
        assert_eq!(fs.offset(), 3);
    }

    #[test]
    fn ramfs_read_to_end() {
        let mut fs = RamFs::with_data(vec![1, 2, 3]);
        let mut buf = [0u8; 10];
        let n = fs.read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(&buf[..n], &[1, 2, 3]);
        assert_eq!(fs.offset(), 3);
    }

    #[test]
    fn ramfs_read_past_end() {
        let mut fs = RamFs::with_data(vec![1, 2]);
        fs.seek(10); // offset 超出文件长度
        let mut buf = [0u8; 5];
        let n = fs.read(&mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn ramfs_read_empty() {
        let mut fs = RamFs::new();
        let mut buf = [0u8; 5];
        let n = fs.read(&mut buf).unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn ramfs_write_basic() {
        let mut fs = RamFs::new();
        let n = fs.write(&[10, 20, 30]).unwrap();
        assert_eq!(n, 3);
        assert_eq!(fs.size(), 3);
        assert_eq!(fs.offset(), 3);
    }

    #[test]
    fn ramfs_write_and_read_back() {
        let mut fs = RamFs::new();
        fs.write(&[1, 2, 3, 4, 5]).unwrap();
        fs.seek(0);
        let mut buf = [0u8; 5];
        fs.read(&mut buf).unwrap();
        assert_eq!(buf, [1, 2, 3, 4, 5]);
    }

    #[test]
    fn ramfs_write_beyond_end() {
        let mut fs = RamFs::new();
        fs.seek(5);
        fs.write(&[99]).unwrap();
        assert_eq!(fs.size(), 6);
        // offset 0..4 应为 0 填充
        fs.seek(0);
        let mut buf = [0u8; 6];
        fs.read(&mut buf).unwrap();
        assert_eq!(buf, [0, 0, 0, 0, 0, 99]);
    }

    #[test]
    fn ramfs_write_empty() {
        let mut fs = RamFs::new();
        let n = fs.write(&[]).unwrap();
        assert_eq!(n, 0);
        assert_eq!(fs.size(), 0);
    }
}
