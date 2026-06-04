//! # FdTable 槽位复用参考实现
//!
//! 测试文件描述符表的槽位复用逻辑。
//!
//! ## 教学概念
//! 当一个 fd 被 close() 后，它的槽位变成 None。
//! 下一次 open() 应该优先复用这个空闲槽位，
//! 而不是一直追加到数组末尾。
//! 这避免了 fd 表无限增长。

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fs::ramfs::RamFs;

    #[test]
    fn fd_table_reuse_slot() {
        let mut table = FdTable::new();
        let fd1 = table.open(Box::new(RamFs::new()));
        table.close(fd1).unwrap();
        let fd2 = table.open(Box::new(RamFs::new()));
        // 关闭后重新打开，应复用同一槽位
        assert_eq!(fd1, fd2);
    }
}
