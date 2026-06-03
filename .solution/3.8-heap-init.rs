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
pub unsafe fn init_heap(start: usize, size: usize) {
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
