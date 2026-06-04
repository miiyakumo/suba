// 集成测试参考实现
// 放在 kernel/src/fs/mod.rs 的 #[cfg(test)] mod tests 块中

/// 集成测试：完整的 VFS 文件操作流程
///
/// 验证 RamFs + FdTable + VfsFile trait 的协作：
/// 创建文件 → 写入数据 → 读取验证
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

    // 4. 通过 VfsFile 接口读取（从当前 offset，此时已在末尾）
    let mut buf = [0u8; 20];
    let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
    assert_eq!(n, 0); // offset 在 write 之后已到末尾
}

/// 集成测试：使用 with_data 预填充 + FdTable 读取
#[test]
fn vfs_integration_with_data() {
    let mut table = FdTable::new();

    let file = Box::new(RamFs::with_data(vec![1, 2, 3, 4, 5]));
    let fd = table.open(file);

    let mut buf = [0u8; 3];
    let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
    assert_eq!(n, 3);
    assert_eq!(buf, [1, 2, 3]);

    let n = table.get_mut(fd).unwrap().read(&mut buf).unwrap();
    assert_eq!(n, 2);
    assert_eq!(&buf[..n], &[4, 5]);
}

/// 集成测试：多文件描述符同时操作
#[test]
fn vfs_integration_multiple_fds() {
    let mut table = FdTable::new();

    let fd1 = table.open(Box::new(RamFs::with_data(vec![10, 20])));
    let fd2 = table.open(Box::new(RamFs::with_data(vec![30, 40, 50])));

    let mut buf1 = [0u8; 2];
    let mut buf2 = [0u8; 3];
    table.get_mut(fd1).unwrap().read(&mut buf1).unwrap();
    table.get_mut(fd2).unwrap().read(&mut buf2).unwrap();

    assert_eq!(buf1, [10, 20]);
    assert_eq!(buf2, [30, 40, 50]);

    table.close(fd1).unwrap();
    assert_eq!(table.get(fd2).unwrap().size(), 3);
}
