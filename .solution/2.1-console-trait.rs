/// 控制台 I/O trait。
///
/// 提供最基本的字符级输入输出能力。
/// Mock 实现直接使用宿主终端，RISC-V 实现使用 UART MMIO。
pub trait Console {
    /// 输出一个字节
    fn putchar(c: u8);

    /// 读取一个字节，无输入时返回 None
    fn getchar() -> Option<u8>;

    /// 输出字符串
    fn puts(s: &str) {
        for b in s.bytes() {
            Self::putchar(b);
        }
    }
}
