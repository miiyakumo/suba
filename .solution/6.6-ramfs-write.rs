// RamFs::write 参考实现
// 放在 impl VfsFile for RamFs 块中

fn write(&mut self, buf: &[u8]) -> Result<usize, ()> {
    // 空写入直接返回
    if buf.is_empty() {
        return Ok(0);
    }
    // 如果 offset 超出当前长度，用 0 填充（模拟 lseek 到文件末尾之后）
    if self.offset > self.data.len() {
        self.data.resize(self.offset, 0);
    }
    // 扩展 data 以容纳新数据
    let end = self.offset + buf.len();
    if end > self.data.len() {
        self.data.resize(end, 0);
    }
    // 写入数据
    self.data[self.offset..end].copy_from_slice(buf);
    self.offset = end;
    Ok(buf.len())
}
