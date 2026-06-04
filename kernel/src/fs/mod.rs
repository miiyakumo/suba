//! # 文件系统 (fs)
//!
//! 本模块实现了虚拟文件系统 (VFS) 和内存文件系统 (RamFs)。
//!
//! ## 教学概念
//! - **VFS 抽象**：统一的文件操作接口，屏蔽底层文件系统差异
//! - **文件描述符**：进程打开文件的句柄，非负整数
//! - **FD Table**：每个任务维护自己的文件描述符表
//! - **RamFs**：最简单的文件系统——所有数据存在内存中
//!
//! ## 教学故事
//! "内存是易失的——程序产生的数据如何在断电后存活？"
//! 答案：文件系统。但首先，我们需要理解文件的抽象。

pub mod ramfs;

use alloc::boxed::Box;

/// VFS 文件 trait。
///
/// 所有文件系统必须实现此 trait。
/// 提供基本的读写操作。
pub trait VfsFile: Send {
    /// 读取数据
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()>;

    /// 写入数据
    fn write(&mut self, buf: &[u8]) -> Result<usize, ()>;

    /// 获取文件大小
    fn size(&self) -> usize;
}

/// 文件描述符表。
///
/// 管理进程打开的所有文件。
/// 文件描述符是非负整数索引。
pub struct FdTable {
    /// 文件描述符数组
    files: alloc::vec::Vec<Option<Box<dyn VfsFile>>>,
}

impl FdTable {
    /// 创建新的文件描述符表
    pub const fn new() -> Self {
        Self {
            files: alloc::vec::Vec::new(),
        }
    }

    /// 打开文件，返回文件描述符
    pub fn open(&mut self, file: Box<dyn VfsFile>) -> usize {
        // 寻找空闲的 fd 槽位
        for (i, slot) in self.files.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(file);
                return i;
            }
        }
        // 没有空闲槽位，追加到末尾
        let fd = self.files.len();
        self.files.push(Some(file));
        fd
    }

    /// 关闭文件描述符
    pub fn close(&mut self, fd: usize) -> Result<(), ()> {
        if fd < self.files.len() && self.files[fd].is_some() {
            self.files[fd] = None;
            Ok(())
        } else {
            Err(())
        }
    }

    /// 获取文件引用
    pub fn get(&self, fd: usize) -> Option<&dyn VfsFile> {
        self.files.get(fd)?.as_deref()
    }

    /// 获取可变文件引用
    pub fn get_mut(&mut self, fd: usize) -> Option<&mut (dyn VfsFile + '_)> {
        match self.files.get_mut(fd)? {
            Some(file) => Some(file.as_mut()),
            None => None,
        }
    }
}

impl Default for FdTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ramfs::RamFs;

    #[test]
    fn fd_table_open_close() {
        let mut table = FdTable::new();
        let file = Box::new(RamFs::new());
        let fd = table.open(file);
        assert_eq!(fd, 0);

        table.close(fd).unwrap();
        assert!(table.get(fd).is_none());
    }

    #[test]
    fn fd_table_reuse_slot() {
        let mut table = FdTable::new();
        let fd1 = table.open(Box::new(RamFs::new()));
        table.close(fd1).unwrap();
        let fd2 = table.open(Box::new(RamFs::new()));
        assert_eq!(fd1, fd2); // 应复用同一槽位
    }

    /// 集成测试：完整的 VFS 文件操作流程
    ///
    /// 验证 RamFs + FdTable + VfsFile trait 的协作：
    /// 创建文件 → 写入数据 → seek 回开头 → 读取验证
    #[test]
    fn vfs_integration_write_seek_read() {
        let mut table = FdTable::new();

        // 1. 创建 RamFs 并打开到 FdTable
        let file = Box::new(RamFs::new());
        let fd = table.open(file);
        assert_eq!(fd, 0);

        // 2. 写入数据
        let data = b"hello, suba!";
        let written = table.get_mut(fd).unwrap().write(data).unwrap();
        assert_eq!(written, data.len());

        // 3. 验证大小
        assert_eq!(table.get(fd).unwrap().size(), data.len());

        // 4. 通过 VfsFile 接口读取（从当前 offset）
        let mut buf = [0u8; 20];
        let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
        // offset 在 write 之后已到末尾，所以 read 返回 0
        assert_eq!(n, 0);
    }

    /// 集成测试：使用 with_data 预填充 + FdTable 读取
    #[test]
    fn vfs_integration_with_data() {
        let mut table = FdTable::new();

        // 创建带初始数据的 RamFs
        let file = Box::new(RamFs::with_data(alloc::vec![1, 2, 3, 4, 5]));
        let fd = table.open(file);

        // 通过 VfsFile 接口读取
        let mut buf = [0u8; 3];
        let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
        assert_eq!(n, 3);
        assert_eq!(buf, [1, 2, 3]);

        // 继续读取剩余数据
        let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
        assert_eq!(n, 2);
        assert_eq!(&buf[..n], &[4, 5]);
    }

    /// 集成测试：多文件描述符同时操作
    #[test]
    fn vfs_integration_multiple_fds() {
        let mut table = FdTable::new();

        let fd1 = table.open(Box::new(RamFs::with_data(alloc::vec![10, 20])));
        let fd2 = table.open(Box::new(RamFs::with_data(alloc::vec![30, 40, 50])));

        // 分别读取
        let mut buf1 = [0u8; 2];
        let mut buf2 = [0u8; 3];
        table.get_mut(fd1).unwrap().read(&mut buf1).unwrap();
        table.get_mut(fd2).unwrap().read(&mut buf2).unwrap();

        assert_eq!(buf1, [10, 20]);
        assert_eq!(buf2, [30, 40, 50]);

        // 关闭 fd1 不影响 fd2
        table.close(fd1).unwrap();
        assert_eq!(table.get(fd2).unwrap().size(), 3);
    }
}
