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
mod driver;
mod util;

use core::arch::asm;
use core::sync::atomic::{AtomicUsize, Ordering};
use suba_kernel::arch::CpuOps;
use suba_kernel::driver::Console;
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

/// 向控制台输出一个字符
///
/// 使用 `UartConsole` 的 `Console` trait 实现，通过 UART MMIO 输出。
pub fn uart_putchar(c: u8) {
    driver::uart::UartConsole::putchar(c);
}

/// 向控制台输出字符串
///
/// 使用 `UartConsole` 的 `Console` trait 实现，包含 `\n` → `\r\n` 转换。
pub fn uart_puts(s: &str) {
    driver::uart::UartConsole::puts(s);
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

    // ---- Step 2.5: 初始化物理帧分配器 ----
    // 帧分配器管理堆区域之后的物理内存，用于分配用户页表和用户内存页
    let frame_alloc_start = heap_start + heap::HEAP_SIZE;
    let frame_alloc_end = suba_kernel::mm::address::PHYS_MEMORY_START
        + suba_kernel::mm::address::PHYS_MEMORY_SIZE;
    // SAFETY: 单线程启动阶段，仅调用一次
    unsafe {
        arch::riscv64::page::init_frame_allocator(frame_alloc_start, frame_alloc_end);
    }
    uart_puts("[suba] frame allocator initialized\n");

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

    // ---- Step 4.5: 初始化中断控制器 ----
    // 初始化 CLINT 时钟中断：设置第一次 timer 并使能 sie.STIE
    driver::clint::init();
    uart_puts("[suba] CLINT initialized (timer interrupt)\n");

    // 初始化 PLIC 外部中断：配置 UART0 路由到 S-mode，使能 sie.SEIE
    driver::plic::init();
    uart_puts("[suba] PLIC initialized (UART0 → S-mode)\n");

    // ---- Step 4.6: 启用全局中断 ----
    // TODO(student): 设置 sstatus.SIE = 1，允许 CPU 响应 S-mode 中断
    //
    // ## 教学概念：中断使能的两级控制
    //
    // RISC-V 中断使能有两级：
    // 1. **sie 寄存器**：按中断类型使能（STIE=时钟, SEIE=外部, SSIE=软件）
    //    - 已在 clint::init() 和 plic::init() 中设置
    // 2. **sstatus.SIE**：S-mode 全局中断总开关
    //    - SIE=1: 允许 CPU 响应已使能的中断
    //    - SIE=0: 忽略所有 S-mode 中断
    //
    // 两级都打开，中断才能到达 CPU。这提供了灵活的中断控制：
    // - 关闭 sstatus.SIE：临时屏蔽所有中断（临界区保护）
    // - 关闭 sie.STIE：仅禁用时钟中断
    //
    // ## 教学概念：中断优先级
    //
    // RISC-V 硬件中断优先级（从高到低）：
    //   1. Supervisor software interrupt (ssi)
    //   2. Supervisor timer interrupt (sti)   ← 时钟中断优先级更高
    //   3. Supervisor external interrupt (sei) ← 外部中断优先级较低
    //
    // 当多个中断同时挂起时，CPU 总是先处理优先级最高的。
    // 这意味着时钟中断可以抢占外部中断处理，保证调度的实时性。
    arch::riscv64::Riscv64CpuOps::enable_interrupts();
    uart_puts("[suba] interrupts enabled (sstatus.SIE=1)\n");

    // ---- Step 5: 页表测试 ----
    uart_puts("[suba] running page table tests...\n");
    arch::riscv64::page::run_tests();

    // ---- Step 5.5: 用户栈创建测试 ----
    // 创建用户地址空间并映射用户栈，验证栈可用
    uart_puts("[suba] creating user stack...\n");
    match arch::riscv64::page::UserAddrSpace::new() {
        Ok(user_space) => {
            match user_space.map_user_stack() {
                Ok(stack_top) => {
                    uart_puts("[suba] user stack mapped: top=");
                    arch::riscv64::page::print_hex(stack_top);
                    uart_puts(", size=8MB\n");
                }
                Err(_) => {
                    uart_puts("[suba] WARN: user stack mapping failed\n");
                }
            }
        }
        Err(_) => {
            uart_puts("[suba] WARN: user addr space creation failed\n");
        }
    }

    // ---- Step 6: 初始化任务系统 ----
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
