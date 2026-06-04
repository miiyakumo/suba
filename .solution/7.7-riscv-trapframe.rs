//! RISC-V TrapFrame 参考实现
//!
//! #[repr(C)] 结构体，包含 34 个 usize 字段：
//! - sepc (offset 0)
//! - x1-x31 (offset 8-248)
//! - sstatus (offset 256)
//! - kernel_sp (offset 264)
//!
//! 实现 HwTrapFrame 和 SyscallFrame trait。
//!
//! 关键实现要点：
//! - set_kernel_trap_frame: 设置 SPP=1, SPIE=1, SIE=0
//! - syscall_id: 读取 x17_a7
//! - set_ret: 写入 x10_a0
//! - zero_init: 使用 core::mem::zeroed()
