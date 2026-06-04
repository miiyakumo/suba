//! # 系统调用号定义
//!
//! 参考实现：系统调用号常量定义。
//!
//! ## 教学概念
//! 每个系统调用有一个唯一编号，用户程序通过编号请求内核服务。
//! RISC-V Linux 的系统调用号与 ARM64 相同。

/// 系统调用号定义
pub mod number {
    /// 退出当前任务
    pub const SYS_EXIT: usize = 93;
    /// 让出 CPU
    pub const SYS_YIELD: usize = 124;
    /// 读取文件描述符
    pub const SYS_READ: usize = 63;
    /// 写入文件描述符
    pub const SYS_WRITE: usize = 64;
    /// 打开文件
    pub const SYS_OPEN: usize = 1024;
    /// 关闭文件描述符
    pub const SYS_CLOSE: usize = 57;
    /// 调整堆大小
    pub const SYS_SBRK: usize = 214;
    /// 获取 PID
    pub const SYS_GETPID: usize = 172;
}
