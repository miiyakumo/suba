//! # FdTable 参考实现
//!
//! 文件描述符表——管理进程打开的所有文件。
//!
//! ## 教学概念
//! - 文件描述符 (fd) 是非负整数，是进程访问文件的句柄
//! - fd 0 = stdin, fd 1 = stdout, fd 2 = stderr（约定）
//! - FdTable 使用 Vec<Option<Box<dyn VfsFile>>> 存储
//! - open() 返回最小的可用 fd
//! - close() 释放 fd 槽位

use alloc::boxed::Box;

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
    pub fn new() -> Self {
        Self {
            files: alloc::vec::Vec::new(),
        }
    }

    /// 打开文件，返回文件描述符
    ///
    /// 优先复用已关闭的槽位，否则追加到末尾。
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
    ///
    /// 释放槽位，使其可被后续 open() 复用。
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
