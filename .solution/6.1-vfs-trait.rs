//! # VfsFile trait 参考实现
//!
//! 虚拟文件系统的统一接口。
//!
//! ## 教学概念
//! VFS (Virtual File System) 是操作系统中文件系统的抽象层。
//! 所有具体的文件系统（RamFs, Ext4, Fat32...）都实现这个 trait，
//! 使得上层代码可以用统一的方式操作不同类型的文件系统。

/// VFS 文件 trait。
///
/// 所有文件系统必须实现此 trait。
/// 提供基本的读写操作。
pub trait VfsFile: Send {
    /// 从当前偏移量读取数据到 buf
    ///
    /// 返回实际读取的字节数。到达文件末尾时返回 0。
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()>;

    /// 从当前偏移量写入 buf 中的数据
    ///
    /// 返回实际写入的字节数。
    fn write(&mut self, buf: &[u8]) -> Result<usize, ()>;

    /// 获取文件大小（字节）
    fn size(&self) -> usize;
}
