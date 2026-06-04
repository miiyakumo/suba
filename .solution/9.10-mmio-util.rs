// .solution/9.10-mmio-util.rs — MMIO 读写辅助工具参考实现
//
// 本文件是 feature 9.10 的参考实现。
// 学生应在 os/src/util/mod.rs 中实现 MMIO 辅助函数。
//
// ## 实现要点
//
// 1. read_u8/u16/u32/u64 — 安全封装 read_volatile
// 2. write_u8/u16/u32/u64 — 安全封装 write_volatile
// 3. set_bits_u32 — 读-修改-写（OR 操作）
// 4. clear_bits_u32 — 读-修改-写（AND NOT 操作）
//
// ## 教学概念：volatile 读写
//
// 普通内存读写可以被编译器优化（合并、重排、消除），
// 但 MMIO 地址映射到硬件寄存器，每次读写都有副作用。
// `read_volatile` / `write_volatile` 保证每次访问都到达硬件。
//
// ## 教学概念：unsafe 封装
//
// MMIO 操作底层是 unsafe 的（地址必须有效、有硬件副作用）。
// 我们用 unsafe 函数封装，上层驱动调用时只需确保地址正确。

use core::ptr::{read_volatile, write_volatile};

// MMIO 读取

pub unsafe fn read_u8(addr: usize) -> u8 {
    unsafe { read_volatile(addr as *const u8) }
}

pub unsafe fn read_u16(addr: usize) -> u16 {
    unsafe { read_volatile(addr as *const u16) }
}

pub unsafe fn read_u32(addr: usize) -> u32 {
    unsafe { read_volatile(addr as *const u32) }
}

pub unsafe fn read_u64(addr: usize) -> u64 {
    unsafe { read_volatile(addr as *const u64) }
}

// MMIO 写入

pub unsafe fn write_u8(addr: usize, val: u8) {
    unsafe { write_volatile(addr as *mut u8, val) }
}

pub unsafe fn write_u16(addr: usize, val: u16) {
    unsafe { write_volatile(addr as *mut u16, val) }
}

pub unsafe fn write_u32(addr: usize, val: u32) {
    unsafe { write_volatile(addr as *mut u32, val) }
}

pub unsafe fn write_u64(addr: usize, val: u64) {
    unsafe { write_volatile(addr as *mut u64, val) }
}

// 读-修改-写

pub unsafe fn set_bits_u32(addr: usize, mask: u32) {
    unsafe {
        let old = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old | mask);
    }
}

pub unsafe fn clear_bits_u32(addr: usize, mask: u32) {
    unsafe {
        let old = read_volatile(addr as *const u32);
        write_volatile(addr as *mut u32, old & !mask);
    }
}
