//! os crate — RISC-V 硬件后端
//!
//! 这是 suba 内核的二进制入口，负责：
//! 1. entry.S: 设置栈、清零 BSS、跳转 rust_main
//! 2. rust_main: 初始化内核子系统，进入主循环
//!
//! ## 启动流程
//! ```text
//! OpenSBI → entry.S (_start) → rust_main()
//!   ├── 打印启动信息
//!   ├── 初始化内核堆
//!   ├── 初始化任务系统
//!   ├── 创建第一个内核任务
//!   └── 启动调度（进入 idle 循环）
//! ```

#![no_std]
#![no_main]

extern crate alloc;

core::arch::global_asm!(include_str!("entry.S"));

mod arch;
mod power;
mod debug_console;

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};
use suba_kernel::mm::heap;
use suba_kernel::task::{RoundRobinScheduler, TaskManager};

// ---------------------------------------------------------------------------
// 全局堆分配器（Bump Allocator）
// ---------------------------------------------------------------------------

/// 简单的 Bump 分配器。
///
/// ## 教学概念
/// `#![no_std]` 环境没有标准库提供的 `malloc`/`free`，
/// 需要自己实现 `GlobalAlloc` trait 来提供动态内存分配。
///
/// Bump 分配器是最简单的分配策略：
/// - 维护一个"水位线"指针，每次分配向上推进
/// - 不支持 free（或仅支持整体回退）
/// - 适合内核启动阶段的短期分配
///
/// 后续可替换为更复杂的分配器（如 buddy、slab）。
struct BumpAllocator {
    /// 堆起始地址
    start: AtomicUsize,
    /// 堆大小
    size: AtomicUsize,
    /// 当前水位线（已分配到的位置）
    current: AtomicUsize,
}

impl BumpAllocator {
    /// 创建未初始化的分配器
    const fn new() -> Self {
        Self {
            start: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
            current: AtomicUsize::new(0),
        }
    }

    /// 初始化分配器
    ///
    /// # Safety
    ///
    /// 必须在单线程启动阶段调用一次。
    unsafe fn init(&self, start: usize, size: usize) {
        self.start.store(start, Ordering::Relaxed);
        self.size.store(size, Ordering::Relaxed);
        self.current.store(start, Ordering::Relaxed);
    }
}

// SAFETY: BumpAllocator 的分配操作是原子的（AtomicUsize）
unsafe impl core::alloc::GlobalAlloc for BumpAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let size = layout.size();
        let align = layout.align();
        let start = self.start.load(Ordering::Relaxed);
        let heap_size = self.size.load(Ordering::Relaxed);

        loop {
            let old = self.current.load(Ordering::Relaxed);
            let aligned = (old + align - 1) & !(align - 1);
            let new = aligned + size;

            if new > start + heap_size {
                // 堆空间不足
                return core::ptr::null_mut();
            }

            match self.current.compare_exchange_weak(
                old,
                new,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => return aligned as *mut u8,
                Err(_) => continue, // 重试
            }
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: core::alloc::Layout) {
        // Bump 分配器不支持单次释放
    }
}

#[global_allocator]
static GLOBAL_ALLOC: BumpAllocator = BumpAllocator::new();

/// 向 QEMU UART (NS16550A) 输出一个字符
///
/// UART0 基地址 0x1000_0000，THR 偏移 0，LSR 偏移 5
/// LSR bit 5 (THR Empty) 为 1 时才能写入
#[inline(always)]
pub fn uart_putchar(c: u8) {
    unsafe {
        asm!(
        "li t0, 0x10000000",   // UART base
        "1:",
        "lb t1, 5(t0)",        // LSR = base + 5
        "andi t1, t1, 0x20",   // THR empty?
        "beqz t1, 1b",         // 等待可发送
        "sb {0}, 0(t0)",       // 写入字符
        in(reg) c,
        out("t0") _,
        out("t1") _,
        );
    }
}

/// 向 UART 输出字符串
pub fn uart_puts(s: &str) {
    for b in s.bytes() {
        uart_putchar(b);
    }
}

/// 内核 Rust 入口
///
/// 由 entry.S 调用，此时：
/// - 栈已设置（sp → boot_stack_top）
/// - BSS 已清零（汇编级）
/// - 中断已关闭（OpenSBI 默认）
///
/// ## 启动步骤
/// 1. 打印启动横幅
/// 2. 初始化内核堆（2MB）
/// 3. 设置陷阱向量（stvec → trap_entry）
/// 4. 初始化任务管理器和调度器
/// 5. 创建 idle 任务（PID 1）
/// 6. 启动调度，进入 idle 循环
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // ---- Step 1: 打印启动信息 ----
    uart_puts("\n[suba] booting on RISC-V...\n");

    // ---- Step 2: 初始化内核堆 ----
    // 堆区域紧跟在内核镜像之后，大小 2MB
    unsafe extern "C" {
        fn ekernel();
    }
    let heap_start = ekernel as usize;
    // SAFETY: 单线程启动阶段，仅调用一次
    unsafe {
        heap::init_heap(heap_start, heap::HEAP_SIZE);
    }
    // 初始化全局分配器（供 alloc crate 使用）
    // SAFETY: 单线程启动阶段，仅调用一次
    unsafe { GLOBAL_ALLOC.init(heap_start, heap::HEAP_SIZE) };
    uart_puts("[suba] heap initialized\n");

    // ---- Step 3: 初始化内核页表 ----
    // 创建 SV39 身份映射页表并激活分页
    // 这将建立虚拟地址到物理地址的翻译（当前 VA=PA）
    arch::riscv64::init_kernel_page_table();
    uart_puts("[suba] kernel page table activated (SV39)\n");

    // ---- Step 4: 设置陷阱向量 ----
    // TODO(student): 将 stvec CSR 设置为 trap_entry 的地址
    // 发生异常/中断时 CPU 会跳转到 stvec 指向的地址
    // 理解: stvec 是 RISC-V 的陷阱向量寄存器 (类似 x86 的 IDTR)
    // init_trap() 写入 stvec: csrw stvec, trap_entry
    arch::riscv64::init_trap();
    uart_puts("[suba] trap vector set\n");

    // ---- Step 4: 初始化任务系统 ----
    let mut tm = TaskManager::new();
    let mut sched = RoundRobinScheduler::new();
    uart_puts("[suba] task system ready\n");

    // ---- Step 4: 创建 idle 任务 ----
    // idle 任务的入口是 idle_loop 函数，栈使用 boot_stack
    unsafe extern "C" {
        fn boot_stack_top();
    }
    let idle_task = tm.create_task(
        idle_loop as usize,
        boot_stack_top as usize,
        0, // 无用户栈
    );
    sched.enqueue(idle_task);
    uart_puts("[suba] idle task created (PID 1)\n");

    // ---- Step 5: 启动调度 ----
    uart_puts("[suba] starting scheduler...\n");

    // 从调度器取出第一个任务并执行
    // 当前阶段：直接进入 idle 循环（上下文切换需要 Arch trait 实现）
    if let Some(task) = sched.next() {
        let t = task.lock();
        uart_puts("[suba] scheduling PID ");
        uart_putchar(b'0' + t.pid as u8);
        uart_putchar(b'\n');
    }

    // TODO(student): 实现真正的上下文切换
    // 需要 RISC-V Arch trait 实现后，调用：
    //   unsafe { ArchImpl::context_switch(current_ctx, next_ctx) };
    idle_loop();
}

/// idle 循环 — 无任务可调度时 CPU 执行此函数
///
/// 在后续实现中，这将成为 idle 任务的入口点：
/// 1. 开启中断（wfi 前）
/// 2. wfi 等待中断
/// 3. 循环
fn idle_loop() -> ! {
    uart_puts("[suba] entering idle loop\n");
    loop {
        // SAFETY: wfi 是特权指令，等待中断唤醒
        unsafe { asm!("wfi") }
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Phase 2 后续 feature 会添加 panic 输出
    power::shutdown(false);
}
