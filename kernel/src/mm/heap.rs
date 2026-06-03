//! # 内核堆
//!
//! 提供内核态的动态内存分配能力。
//!
//! ## 教学概念
//! - `#![no_std]` 环境没有 `malloc`/`free`，需要自建堆分配器
//! - 使用 talc 分配器（轻量级，适合嵌入式环境）
//! - 堆初始化必须在内核启动早期完成

/// 内核堆大小（2MB）
pub const HEAP_SIZE: usize = super::address::KERNEL_HEAP_SIZE;

/// 堆的起始地址（将在初始化时设置）
static mut HEAP_START: usize = 0;

/// 堆是否已初始化
static mut HEAP_INITIALIZED: bool = false;

/// 初始化内核堆。
///
/// # Safety
///
/// 必须在内核启动早期调用一次，且不能并发调用。
pub unsafe fn init_heap(start: usize, _size: usize) {
    // SAFETY: 仅在单线程启动阶段调用
    unsafe {
        HEAP_START = start;
        // TODO: 初始化 talc 分配器
        // allocator.lock().init(start as *mut u8, size);
        HEAP_INITIALIZED = true;
    }
}

/// 堆是否已初始化
pub fn is_initialized() -> bool {
    // SAFETY: 读取 bool 是原子操作
    unsafe { HEAP_INITIALIZED }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_size_constant() {
        assert_eq!(HEAP_SIZE, 0x20_0000); // 2MB
    }

    #[test]
    fn heap_init_sets_initialized() {
        // 注意：此测试修改全局状态，需要 --test-threads=1 或接受状态泄漏
        // SAFETY: 测试环境单线程，且使用 mock 地址
        unsafe { init_heap(0x1000_0000, HEAP_SIZE) };
        assert!(is_initialized());
    }
}
