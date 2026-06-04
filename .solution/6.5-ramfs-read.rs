// RamFs::read 参考实现
// 放在 impl VfsFile for RamFs 块中

fn read(&mut self, buf: &mut [u8]) -> Result<usize, ()> {
    // 如果 offset 已经超出文件末尾，返回 0
    if self.offset >= self.data.len() {
        return Ok(0);
    }
    // 计算可读取的字节数：缓冲区大小 和 剩余数据量 的较小值
    let remaining = self.data.len() - self.offset;
    let to_read = buf.len().min(remaining);
    // 从 data 复制到 buf
    buf[..to_read].copy_from_slice(&self.data[self.offset..self.offset + to_read]);
    // 更新 offset
    self.offset += to_read;
    Ok(to_read)
}
