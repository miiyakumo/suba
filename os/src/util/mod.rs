//! # MMIO 读写辅助工具
//!
//! 本模块提供 MMIO (Memory-Mapped I/O) 寄存器的安全读写封装。
//!
//! ## 教学概念：为什么需要 volatile？
//!
//! 普通内存读写可以被编译器优化：
//! - **合并**：多次写入合并为一次
//! - **重排**：改变读写顺序
//! - **消除**：删除"无用"的读写
//!
//! 但 MMIO 地址映射到硬件寄存器，每次读写都有副作用：
//! - 读取 UART 接收缓冲区 → 取出数据（读一次就没了）
//! - 写入 UART 发送缓冲区 → 触发硬件发送
//!
//! `volatile` 告诉编译器：
//! "这次访问必须真正发生，不能优化掉"。
//!
//! ## 教学概念：unsafe 的必要性
//!
//! MMIO 操作是 unsafe 的，因为：
//! 1. 地址必须是有效的 MMIO 区域（否则访问普通内存可能 crash）
//! 2. 读写可能有硬件副作用（影响设备状态）
//! 3. 时序敏感（某些寄存器需要特定顺序访问）
//!
//! 我们用 `unsafe` 函数封装底层操作，上层驱动只需调用安全接口。

#![allow(dead_code)]

use core::ptr::{read_volatile, write_volatile};

// ============================================================================
// MMIO 读取
// ============================================================================

/// 从 MMIO 地址读取 u8
///
/// # Safety
///
/// `addr` 必须是有效的 MMIO 地址，映射到硬件寄存器。
///
/// ## 教学概念：read_volatile
///
/// `read_volatile(ptr)` 保证：
/// 1. 每次调用都真正从 `ptr` 读取（不缓存上次结果）
/// 2. 读取顺序与代码顺序一致（不重排）
/// 3. 不会被编译器消除（即使"看起来"没有使用结果）
#[inline]
pub unsafe fn read_u8(addr: usize) -> u8 {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { read_volatile(addr as *const u8) }
}

/// 从 MMIO 地址读取 u16
#[inline]
pub unsafe fn read_u16(addr: usize) -> u16 {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { read_volatile(addr as *const u16) }
}

/// 从 MMIO 地址读取 u32
///
/// 大多数 RISC-V MMIO 寄存器是 32 位宽。
#[inline]
pub unsafe fn read_u32(addr: usize) -> u32 {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { read_volatile(addr as *const u32) }
}

/// 从 MMIO 地址读取 u64
///
/// 某些寄存器（如 PLIC priority）是 64 位宽。
#[inline]
pub unsafe fn read_u64(addr: usize) -> u64 {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { read_volatile(addr as *const u64) }
}

// ============================================================================
// MMIO 写入
// ============================================================================

/// 向 MMIO 地址写入 u8
///
/// # Safety
///
/// `addr` 必须是有效的 MMIO 地址。
///
/// ## 教学概念：write_volatile
///
/// `write_volatile(ptr, val)` 保证：
/// 1. 每次调用都真正写入 `ptr`（不省略"重复"写入）
/// 2. 写入顺序与代码顺序一致
/// 3. 写入值精确是 `val`（不优化位操作）
#[inline]
pub unsafe fn write_u8(addr: usize, val: u8) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { write_volatile(addr as *mut u8, val) }
}

/// 向 MMIO 地址写入 u16
#[inline]
pub unsafe fn write_u16(addr: usize, val: u16) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { write_volatile(addr as *mut u16, val) }
}

/// 向 MMIO 地址写入 u32
#[inline]
pub unsafe fn write_u32(addr: usize, val: u32) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { write_volatile(addr as *mut u32, val) }
}

/// 向 MMIO 地址写入 u64
#[inline]
pub unsafe fn write_u64(addr: usize, val: u64) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe { write_volatile(addr as *mut u64, val) }
}

// ============================================================================
// 读-修改-写 操作
// ============================================================================

/// 设置 MMIO 寄存器的指定位（OR 操作）
///
/// 读取当前值，与 `mask` 做 OR，写回。
/// 常用于使能寄存器的某个 bit。
///
/// # Safety
///
/// `addr` 必须是有效的 MMIO 地址。
///
/// ## 教学概念：读-修改-写 (Read-Modify-Write)
///
/// 很多 MMIO 寄存器需要"只修改部分 bit"：
/// ```text
/// old = read_volatile(addr)    // 读取当前值
/// new = old | mask             // 设置目标 bit
/// write_volatile(addr, new)    // 写回
/// ```
///
/// 注意：这不是原子操作！如果在读和写之间发生中断，
/// 中断处理函数可能修改同一寄存器，导致覆盖。
/// 需要时应配合中断禁用使用。
#[inline]
pub unsafe fn set_bits_u32(addr: usize, mask: u32) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe {
        let old = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old | mask);
    }
}

/// 清除 MMIO 寄存器的指定位（AND NOT 操作）
///
/// 读取当前值，与 `mask` 的取反做 AND，写回。
/// 常用于禁用寄存器的某个 bit。
///
/// # Safety
///
/// `addr` 必须是有效的 MMIO 地址。
#[inline]
pub unsafe fn clear_bits_u32(addr: usize, mask: u32) {
    // SAFETY: 调用者确保 addr 是有效的 MMIO 地址
    unsafe {
        let old = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old & !mask);
    }
}
