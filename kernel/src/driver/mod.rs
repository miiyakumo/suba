//! # 驱动框架 (driver)
//!
//! 本模块定义了设备驱动的抽象接口。
//!
//! ## 教学概念
//! - **Console trait**：控制台 I/O 抽象，支持字符读写
//! - **Power trait**：电源管理抽象（关机、重启）
//! - 驱动框架的核心思想：接口与实现分离，同一接口可对接 Mock 或真实硬件

pub mod mock;

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

/// 电源管理 trait。
///
/// 提供关机和重启操作。
pub trait Power {
    /// 关机
    fn shutdown() -> !;

    /// 重启
    fn reboot() -> !;
}
