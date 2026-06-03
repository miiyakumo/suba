//! # 地址空间布局常量
//!
//! 定义内核和用户地址空间的布局常量。
//!
//! ## 教学概念
//! RISC-V SV39 使用 39 位虚拟地址，分为：
//! - 用户空间：0x0000_0000_0000_0000 ~ 0x0000_003F_FFFF_FFFF
//! - 内核空间：0xFFFF_FC00_0000_0000 ~ 0xFFFF_FFFF_FFFF_FFFF
//!
//! 常见内存布局：
//! ```text
//! 用户空间:
//!   [0x0, USER_TOP)            -- 用户代码、数据、栈
//!
//! 内核空间:
//!   [KERNEL_BASE, +∞)         -- 内核代码和数据（直接映射）
//!   [MMIO_BASE, +∞)           -- 设备 MMIO 区域
//! ```

/// 用户空间上限（SV39: 256 GB）
pub const USER_TOP: usize = 0x0000_0040_0000_0000;

/// 内核起始虚拟地址（直接映射偏移）
pub const KERNEL_BASE: usize = 0xFFFF_FC00_0000_0000;

/// MMIO 区域起始地址
pub const MMIO_BASE: usize = 0xFFFF_FC10_0000_0000;

/// 内核栈大小（16KB，4 个页）
pub const KERNEL_STACK_SIZE: usize = 4096 * 4;

/// 用户栈大小（默认 8MB）
pub const USER_STACK_SIZE: usize = 0x80_0000;

/// 内核堆大小（2MB）
pub const KERNEL_HEAP_SIZE: usize = 0x20_0000;

/// 物理内存起始地址（QEMU virt 机器）
pub const PHYS_MEMORY_START: usize = 0x8000_0000;

/// 物理内存大小（128MB）
pub const PHYS_MEMORY_SIZE: usize = 0x0800_0000;
