//! os crate — RISC-V 硬件后端
//!
//! 这是 suba 内核的二进制入口，负责：
//! 1. entry.S: 设置栈、清零 BSS、跳转 rust_main
//! 2. rust_main: 初始化内核子系统，加载 init 程序，进入调度
//!
//! ## 完整启动流程
//!
//! ```text
//! OpenSBI (M-mode)
//!   │
//!   └─ ecall → entry.S (_start)
//!        ├── 设置栈指针 (sp → boot_stack_top)
//!        ├── 清零 BSS 段
//!        └── call rust_main()
//!             │
//!             ├─ Step 1:  打印启动横幅
//!             ├─ Step 2:  初始化内核堆 (2MB)
//!             ├─ Step 3:  初始化物理帧分配器
//!             ├─ Step 4:  创建内核页表并激活 SV39
//!             ├─ Step 5:  设置陷阱向量 (stvec → trap_entry)
//!             ├─ Step 6:  初始化中断控制器 (CLINT + PLIC)
//!             ├─ Step 7:  启用全局中断 (sstatus.SIE=1)
//!             ├─ Step 8:  初始化任务系统
//!             ├─ Step 9:  加载 init 程序到用户地址空间
//!             └─ Step 10: 启动调度器，进入 idle 循环
//! ```
//!
//! ## 教学概念：内核启动的分层初始化
//!
//! 内核启动是一个"逐步解锁能力"的过程：
//! - 没有堆 → 不能用 Box/Vec（只能栈上分配和静态变量）
//! - 没有页表 → 所有地址都是物理地址（VA=PA）
//! - 没有中断 → CPU 单线程顺序执行
//! - 没有任务 → 只有一个执行流
//!
//! 每一步初始化都为下一步奠定基础，顺序不能随意调换。
//! 这是理解操作系统启动的关键洞察。

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
use suba_kernel::arch::{Arch, CpuOps};
use suba_kernel::driver::Console;
use suba_kernel::mm::heap;
use suba_kernel::task::{RoundRobinScheduler, TaskManager, TaskState};

// ---------------------------------------------------------------------------
// 全局任务管理器和调度器
// ---------------------------------------------------------------------------

/// 当前正在运行的任务 PID。
///
/// 用于在 trap 处理中识别当前任务（如 exit 时标记任务状态）。
static CURRENT_PID: AtomicUsize = AtomicUsize::new(0);

/// Per-task 内核栈大小（4KB）
///
/// ## 教学概念：为什么每个任务需要独立的内核栈？
///
/// 当时钟中断打断用户态任务时，CPU 切换到内核栈执行 trap 处理。
/// 如果多个任务共享同一个内核栈：
/// - 任务 A 被中断，寄存器保存到栈上
/// - 任务 B 运行并被中断，覆盖任务 A 的数据
/// - 任务 A 恢复时数据已损坏
///
/// 每个任务需要独立的内核栈，确保 trap 处理互不干扰。
/// 内核栈不需要很大——只需容纳 trap 处理的函数调用链。
const TASK_KERNEL_STACK_SIZE: usize = 4096;

/// Per-task 内核栈分配池
///
/// 使用静态数组为每个用户任务分配独立的内核栈。
/// 最多支持 MAX_USER_TASKS 个用户任务。
const MAX_USER_TASKS: usize = 8;

/// 内核栈内存池（每个 4KB，8 个任务 = 32KB）
#[repr(C, align(4096))]
struct KernelStackPool {
    stacks: [[u8; TASK_KERNEL_STACK_SIZE]; MAX_USER_TASKS],
}

/// 内核栈池实例（BSS 段，启动时自动清零）
static mut STACK_POOL: KernelStackPool = KernelStackPool {
    stacks: [[0; TASK_KERNEL_STACK_SIZE]; MAX_USER_TASKS],
};

/// 下一个可分配的栈索引
static STACK_ALLOC_IDX: AtomicUsize = AtomicUsize::new(0);

/// 分配一个内核栈，返回栈顶地址
///
/// 栈顶 = 栈基址 + 栈大小（栈向低地址增长）。
/// 返回的地址是栈空间的最高地址（第一个 push 会写到 addr - 8）。
fn alloc_kernel_stack() -> usize {
    let idx = STACK_ALLOC_IDX.fetch_add(1, Ordering::Relaxed);
    if idx >= MAX_USER_TASKS {
        panic!("[suba] kernel stack pool exhausted");
    }
    // SAFETY: 每个 idx 唯一（原子递增），不会并发访问同一栈
    let base = unsafe { &STACK_POOL.stacks[idx] as *const _ as usize };
    base + TASK_KERNEL_STACK_SIZE
}

/// 全局任务管理器（在 rust_main 中初始化）。
///
/// ## 教学概念：为什么需要全局任务管理器？
///
/// trap_handler 在中断上下文中执行，无法通过参数获取任务管理器。
/// 使用全局静态变量是内核中常见的做法。
/// spin::Once 保证线程安全的一次性初始化。
static TASK_MANAGER: spin::Once<spin::Mutex<TaskManager>> = spin::Once::new();

/// 全局调度器（在 rust_main 中初始化）。
static SCHEDULER: spin::Once<spin::Mutex<RoundRobinScheduler>> = spin::Once::new();

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

// ---------------------------------------------------------------------------
// 系统调用后处理：exit 路径
// ---------------------------------------------------------------------------

/// 系统调用后处理回调 — 处理 exit 系统调用。
///
/// 当用户程序调用 exit (SYS_EXIT) 时，此函数在 dispatch 返回后被调用。
/// 它完成 exit 的后半段工作：
/// 1. 将当前任务状态设为 Exited
/// 2. 调度下一个任务（或关机）
///
/// ## 教学概念：exit 的完整路径
///
/// ```text
/// 用户程序                  内核
/// ┌──────────┐            ┌──────────────────┐
/// │ li a7, 93│  ecall     │ trap_entry       │
/// │ li a0, 0 │ ─────────► │ trap_handler     │
/// │ ecall    │            │   dispatch(tf)   │
/// │          │            │   → sys_exit(0)  │
/// │ (不再执行)│            │   after_syscall  │
/// └──────────┘            │   → mark Exited  │
///                         │   → schedule()   │
///                         │   → 关机或切换    │
///                         └──────────────────┘
/// ```
fn after_syscall_exit(tf: &mut arch::riscv64::TrapFrame) {
    use suba_kernel::syscall::number::SYS_EXIT;

    // 检查是否是 exit 系统调用（a7 = SYS_EXIT = 93）
    if tf.x17_a7 != SYS_EXIT {
        return;
    }

    let exit_code = tf.x10_a0 as i32;
    let pid = CURRENT_PID.load(Ordering::Relaxed);

    uart_puts("[exit] PID ");
    uart_putchar(b'0' + pid as u8);
    uart_puts(" exited with code ");
    // 简单打印退出码（仅支持 0-9）
    if (0..=9).contains(&exit_code) {
        uart_putchar(b'0' + exit_code as u8);
    } else {
        uart_putchar(b'?');
    }
    uart_putchar(b'\n');

    // 标记当前任务为 Exited
    if let Some(tm) = TASK_MANAGER.get() {
        let tm = tm.lock();
        if let Some(task) = tm.get_task(pid) {
            let mut task = task.lock();
            task.state = TaskState::Exited;
            task.exit_code = exit_code;
        }
    }

    // 调度下一个任务
    schedule();
}

// ---------------------------------------------------------------------------
// 调度回调（由 clint.rs 时钟中断调用）
// ---------------------------------------------------------------------------

/// 调度函数 — 由 clint.rs 的时钟中断处理调用。
///
/// 返回值：下一个任务的 TrapFrame 指针，如果不需要切换则返回 null。
///
/// ## 教学概念：抢占式调度的触发路径
///
/// ```text
/// 时钟中断 → trap_entry → trap_handler
///   → handle_timer_interrupt()
///     → schedule_fn()
///       → schedule() → context_switch() → NEXT_TRAP_FRAME
///     → 返回 NEXT_TRAP_FRAME
///   → trap_handler 返回
/// → trap.S 检查 NEXT_TRAP_FRAME → 恢复新任务 → sret
/// ```
fn schedule_fn() -> *mut arch::riscv64::TrapFrame {
    schedule();
    arch::riscv64::NEXT_TRAP_FRAME.load(core::sync::atomic::Ordering::Acquire)
}

/// 检查是否所有用户任务已退出。
///
/// 遍历 TASK_MANAGER 中所有非 idle (PID > 1) 的任务，
/// 如果全部处于 Exited 状态，返回 true。
///
/// ## 教学概念：关机检测
///
/// 内核通过定时检查任务状态来决定何时关机：
/// - 当所有用户任务都退出（Exited）时，没有更多工作需要完成
/// - 内核进行最后一次资源回收，然后通过 SBI 调用关闭硬件
/// - 这避免了内核在 QEMU 中"挂死"——永远等待不存在的就绪任务
///
/// idle 任务 (PID=1) 永远处于 Running 状态，不参与检查。
///
/// TODO(student): 关机检测和资源回收
/// 当前实现检测所有用户任务退出后关机。
/// 改进方向：
/// 1. 在关机前遍历 TASK_MANAGER 回收所有任务资源
/// 2. 打印每个任务的退出码和运行时间摘要
/// 3. 调用 power::shutdown 前确保所有 I/O 已刷新
fn all_user_tasks_exited() -> bool {
    if let Some(tm) = TASK_MANAGER.get() {
        let tm = tm.lock();
        let total = tm.len();
        // idle 任务 (PID=1) 总是 Running，只检查 PID >= 2 的用户任务
        for pid in 2..=total {
            if let Some(task) = tm.get_task(pid) {
                if task.lock().state != TaskState::Exited {
                    return false;
                }
            }
        }
        total > 1 // 至少有一个用户任务存在
    } else {
        false
    }
}

/// 调度器 — 选择下一个任务并切换。
///
/// 从调度器取出下一个就绪任务，通过上下文切换跳转到该任务。
/// 如果没有就绪任务且所有用户任务已退出，触发系统关机。
fn schedule() {
    // 从调度器获取下一个就绪任务的信息
    let next_info = if let Some(sched) = SCHEDULER.get() {
        sched.lock().next().and_then(|task| {
            let mut t = task.lock();
            t.state = TaskState::Running;
            let pid = t.pid;
            CURRENT_PID.store(pid, Ordering::Relaxed);

            uart_puts("[sched] PID ");
            uart_putchar(b'0' + pid as u8);
            uart_putchar(b'\n');

            Some((t.context_ptr(), t.trap_frame_ptr, t.page_table_root))
        })
    } else {
        None
    };

    if let Some((next_ctx, trap_frame_ptr, page_table_root)) = next_info {
        // 切换到新任务的页表
        if page_table_root != 0 {
            // SAFETY: page_table_root 是有效的 SV39 页表根 PPN
            unsafe { arch::riscv64::switch_page_table(page_table_root); }
        }

        // 设置 NEXT_TRAP_FRAME：trap.S 在 trap_handler 返回后检查此变量
        if trap_frame_ptr != 0 {
            arch::riscv64::NEXT_TRAP_FRAME.store(
                trap_frame_ptr as *mut arch::riscv64::TrapFrame,
                core::sync::atomic::Ordering::Release,
            );
        }

        // 获取当前任务的上下文指针（用于保存）
        let current_pid = CURRENT_PID.load(Ordering::Relaxed);
        let old_ctx = if let Some(tm) = TASK_MANAGER.get() {
            tm.lock().get_task(current_pid).map(|t| t.lock().context_ptr())
        } else {
            None
        };

        if let Some(old_ctx) = old_ctx {
            // SAFETY: old_ctx 和 next_ctx 来自有效的 Task
            unsafe {
                arch::riscv64::Riscv64Arch::context_switch(old_ctx, next_ctx);
            }
            // context_switch 返回：当前任务被恢复
        }
    } else {
        // TODO(student): 关机流程 — 检测所有任务退出并关闭系统
        // 当调度器队列中没有就绪任务时，检查是否所有用户任务已退出。
        // 如果是，说明内核已经完成了所有工作，应该正常关机。
        //
        // 关机流程：
        // 1. 遍历 TASK_MANAGER 检查所有用户任务状态
        // 2. 所有用户任务退出 → 打印关机摘要
        // 3. 通过 SBI system_reset 关闭 QEMU
        //
        // 参考：power::shutdown(false) 使用 sbi_rt 的 system_reset
        if all_user_tasks_exited() {
            uart_puts("[suba] all user tasks exited, shutting down...\n");
            power::shutdown(false);
        }
    }
}

/// 用户任务入口 trampoline
///
/// 当调度器通过 context_switch 选择一个用户任务时，
/// CPU 跳转到此函数。此函数负责：
/// 1. 从 TASK_MANAGER 获取当前任务的信息
/// 2. 激活任务的页表（page_table_root）
/// 3. 通过 sret 切换到用户模式
/// 用户任务入口 trampoline
///
/// ## 教学概念：forkret 路径
///
/// 新创建的用户任务首次被调度时，context.ra 指向此函数。
/// trampoline 负责：
/// 1. 激活任务的页表（每个用户任务有独立的 SV39 页表）
/// 2. 将 trap_frame_ptr 写入 sscratch（供 trap_entry 保存寄存器）
/// 3. 通过 trap_return 恢复 TrapFrame 并 sret 到用户态
///
/// sscratch 的作用：当用户态发生陷阱时，trap_entry 读取 sscratch
/// 获取 TrapFrame 地址，将全部寄存器保存到该位置。
/// 每个任务的 sscratch 指向其专属的 TrapFrame。
fn user_task_entry() -> ! {
    let pid = CURRENT_PID.load(Ordering::Relaxed);

    let (page_table_root, trap_frame_ptr) =
        if let Some(tm) = TASK_MANAGER.get() {
            let tm = tm.lock();
            if let Some(task) = tm.get_task(pid) {
                let t = task.lock();
                (t.page_table_root, t.trap_frame_ptr)
            } else {
                uart_puts("[trampoline] ERROR: task not found\n");
                power::shutdown(false);
            }
        } else {
            uart_puts("[trampoline] ERROR: TASK_MANAGER not initialized\n");
            power::shutdown(false);
        };

    // 激活任务的页表
    if page_table_root != 0 {
        // SAFETY: page_table_root 是有效的 SV39 页表根 PPN
        unsafe {
            arch::riscv64::switch_page_table(page_table_root);
        }
    }

    // 将 trap_frame_ptr 写入 sscratch
    // 当任务被中断时，trap_entry 从 sscratch 读取 TrapFrame 地址
    // SAFETY: trap_frame_ptr 指向有效的 TrapFrame
    unsafe {
        core::arch::asm!(
            "csrw sscratch, {tf}",
            tf = in(reg) trap_frame_ptr,
            options(nomem, nostack)
        );
    }

    // 通过 trap_return 恢复 TrapFrame 并 sret 到用户态
    // TrapFrame 已在 rust_main 中初始化（setup_user_trap_frame）
    // SAFETY: trap_frame_ptr 指向正确初始化的 TrapFrame
    unsafe {
        arch::riscv64::trap_return(trap_frame_ptr as *mut arch::riscv64::TrapFrame);
    }

    unreachable!("[trampoline] trap_return returned")
}

// ===========================================================================
// 内核主入口 — 完整启动流程
// ===========================================================================

/// 内核 Rust 入口 — 完整启动流程
///
/// 由 entry.S 调用，此时：
/// - 栈已设置（sp → boot_stack_top）
/// - BSS 已清零（汇编级）
/// - 中断已关闭（OpenSBI 默认）
///
/// 本函数按照严格的顺序初始化所有内核子系统，
/// 最终加载 init 程序并启动调度器。
///
/// ## 教学概念：启动顺序为什么重要？
///
/// 每一步初始化都依赖前面的步骤：
/// - 堆初始化需要 BSS 已清零（全局变量为零）
/// - 帧分配器需要堆已初始化（分配器元数据在堆上）
/// - 页表需要帧分配器（分配页表页）
/// - 中断需要页表（MMIO 地址映射）和陷阱向量
/// - 任务系统需要堆和中断（任务结构体分配、时钟中断驱动调度）
/// - 用户程序需要任务系统（创建用户任务）和页表（用户地址空间）
#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // ================================================================
    // Step 1: 打印启动横幅
    // ================================================================
    // 此时 UART 已由 OpenSBI 初始化，可以直接输出
    uart_puts("\n");
    uart_puts("========================================\n");
    uart_puts("  suba kernel — RISC-V 64-bit\n");
    uart_puts("========================================\n");

    // ================================================================
    // Step 2: 初始化内核堆 (2MB)
    // ================================================================
    // 堆区域紧跟在内核镜像之后（ekernel 符号由链接脚本定义）
    // 初始化后才能使用 Box、Vec、Arc 等需要动态分配的类型
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
    uart_puts("[boot] heap initialized (2MB)\n");

    // ================================================================
    // Step 3: 初始化物理帧分配器
    // ================================================================
    // 帧分配器管理堆区域之后的物理内存，用于分配：
    // - 用户页表页
    // - 用户内存页（代码、数据、栈）
    let frame_alloc_start = heap_start + heap::HEAP_SIZE;
    let frame_alloc_end = suba_kernel::mm::address::PHYS_MEMORY_START
        + suba_kernel::mm::address::PHYS_MEMORY_SIZE;
    // SAFETY: 单线程启动阶段，仅调用一次
    unsafe {
        arch::riscv64::page::init_frame_allocator(frame_alloc_start, frame_alloc_end);
    }
    uart_puts("[boot] frame allocator initialized\n");

    // ================================================================
    // Step 4: 创建内核页表并激活 SV39 分页
    // ================================================================
    // 使用 1GB 大页进行身份映射（VA=PA），覆盖 4GB 地址空间
    // 激活 satp 后，所有内存访问都经过页表翻译
    //
    // ## 教学概念：身份映射 (Identity Mapping)
    //
    // 身份映射是最简单的页表配置：虚拟地址 = 物理地址。
    // 这样内核代码在开启分页前后使用相同的地址，无需修改。
    // 缺点是无法利用虚拟内存的隔离和保护能力。
    // 后续可以迁移到高地址映射（VA = PA + offset）。
    arch::riscv64::init_kernel_page_table();
    uart_puts("[boot] kernel page table activated (SV39)\n");

    // ================================================================
    // Step 5: 设置陷阱向量 (stvec → trap_entry)
    // ================================================================
    // stvec 告诉 CPU："发生陷阱时跳转到这个地址"
    // trap_entry 在 trap.S 中实现，保存全部寄存器后调用 trap_handler
    //
    // ## 教学概念：Direct vs Vectored 模式
    //
    // stvec 低 2 位选择陷阱模式：
    // - 0 (Direct): 所有陷阱跳转到基地址（我们使用此模式）
    // - 1 (Vectored): 中断跳转到 base+4*cause，异常跳转到 base
    //
    // Direct 模式更灵活：由软件根据 scause 分发，
    // 而不是由硬件强制跳转到不同地址。
    arch::riscv64::init_trap();
    uart_puts("[boot] trap vector set (stvec → trap_entry)\n");

    // ================================================================
    // Step 6: 初始化中断控制器
    // ================================================================
    // CLINT (Core Local Interruptor): 提供时钟中断和软件中断
    // - 时钟中断是调度的基础：每次 timer 到期触发调度决策
    driver::clint::init();
    uart_puts("[boot] CLINT initialized (timer interrupt)\n");

    // PLIC (Platform-Level Interrupt Controller): 管理外部设备中断
    // - UART0 中断：接收键盘输入
    // - 路由到 S-mode，由内核处理
    driver::plic::init();
    uart_puts("[boot] PLIC initialized (UART0 → S-mode)\n");

    // UART 接收中断使能
    // 设置 IER 的 ERBFI 位（Enable Received Data Available Interrupt）
    // 当用户在键盘上按键时，UART 硬件触发中断：
    // UART → PLIC → CPU (scause=9) → trap_handler → plic::handle_external
    // → uart.handle_interrupt → 数据存入 RX_BUFFER
    //
    // TODO(student): UART 中断输入路径验证
    // 验证完整的 UART 中断输入链路：
    // 1. UART IER 使能接收中断
    // 2. PLIC 路由 UART0 IRQ 到 S-mode
    // 3. 用户按键 → UART 中断 → PLIC → scause=9
    // 4. trap_handler 分发到 plic::handle_external_interrupt
    // 5. plic claim → uart.handle_interrupt → 数据存入 RX_BUFFER
    // 6. sys_read (getchar) 从 RX_BUFFER 读取数据
    // 7. 数据通过 copy_to_user 返回给用户程序
    driver::uart::init_interrupt();
    uart_puts("[boot] UART receive interrupt enabled\n");

    // ================================================================
    // Step 7: 启用全局中断 (sstatus.SIE=1)
    // ================================================================
    // RISC-V 中断使能有两级控制：
    // 1. sie 寄存器：按中断类型使能（已在 Step 6 设置）
    // 2. sstatus.SIE：S-mode 全局中断总开关（现在打开）
    //
    // 两级都打开，中断才能到达 CPU。
    //
    // ## 教学概念：为什么在最后才启用中断？
    //
    // 中断处理函数依赖前面初始化的子系统：
    // - 时钟中断需要任务系统（调度器）
    // - 外部中断需要 PLIC 配置
    // - 陷阱处理需要 stvec 和页表
    //
    // 如果过早启用中断，中断处理函数可能访问未初始化的数据结构。
    arch::riscv64::Riscv64CpuOps::enable_interrupts();
    uart_puts("[boot] interrupts enabled (sstatus.SIE=1)\n");

    // ================================================================
    // Step 8: 初始化任务系统
    // ================================================================
    // TaskManager 管理所有任务的创建和查找
    // RoundRobinScheduler 实现简单的轮转调度
    //
    // ## 教学概念：全局任务管理器
    //
    // 使用 spin::Once 将 TaskManager 和 Scheduler 存储为全局静态变量。
    // 这样 trap_handler 中的系统调用处理可以访问任务管理器
    // （例如 exit 需要标记任务状态为 Exited）。
    TASK_MANAGER.call_once(|| spin::Mutex::new(TaskManager::new()));
    SCHEDULER.call_once(|| spin::Mutex::new(RoundRobinScheduler::new()));

    // 创建 idle 任务（PID=1）
    // idle 任务在无其他可运行任务时执行，使用 wfi 等待中断
    unsafe extern "C" {
        fn boot_stack_top();
    }
    let idle_task = {
        let mut tm = TASK_MANAGER.get().unwrap().lock();
        tm.create_task(
            idle_loop as usize,
            boot_stack_top as usize,
            0, // 无用户栈
        )
    };
    {
        let mut sched = SCHEDULER.get().unwrap().lock();
        sched.enqueue(idle_task);
    }
    uart_puts("[boot] task system ready (idle task PID=1)\n");

    // 注册系统调用后处理回调（exit 路径）
    // SAFETY: 中断尚未在此代码路径中触发（Step 9 之前）
    unsafe {
        arch::riscv64::init_exit_handler(after_syscall_exit);
    }
    uart_puts("[boot] exit handler registered\n");

    // 注册调度函数（由 clint.rs 时钟中断调用）
    // 当时钟中断来自用户态且时间片用完时，clint 调用此函数触发任务切换。
    // SAFETY: 中断尚未在此代码路径中触发（Step 9 之前）
    unsafe {
        driver::clint::init_schedule_fn(schedule_fn);
    }
    uart_puts("[boot] schedule function registered\n");

    // ================================================================
    // Step 9: 加载用户程序并创建多个任务
    // ================================================================
    let init_elf = include_bytes!("user_init.bin");
    let loop_elf = include_bytes!("user_loop.bin");
    uart_puts("[boot] loading user programs...\n");

    // --- 创建用户任务 A (user_init.bin: 立即 exit) ---
    let task_a = match arch::riscv64::load_elf_to_space(init_elf) {
        Ok((user_space, entry, stack_top)) => {
            let root_ppn = user_space.root_ppn();
            // 分配独立的内核栈（每个任务需要独立的内核栈和 TrapFrame）
            let kstack_top = alloc_kernel_stack();
            let task = {
                let mut tm = TASK_MANAGER.get().unwrap().lock();
                tm.create_user_task(user_task_entry as usize, kstack_top, stack_top, root_ppn, entry)
            };
            // 设置 TrapFrame 和 context.sp（用于 context_switch 首次调度）
            //
            // ## 教学概念：内核栈布局
            //
            // ```text
            // kstack_top ─────────────
            //             │ TrapFrame │  ← trap_frame_ptr (272 bytes, sscratch 指向此)
            // kstack_top  │           │
            //   - 272  ───────────────
            //             │ (8 bytes) │  ← sp / kernel_sp (从这里开始)
            // kstack_top  │           │     trap_entry 切换到此，函数调用向下增长
            //   - 280  ───────────────
            //             │  栈空间    │
            //             │  (向下增长)│
            // kstack_base ─────────────
            // ```
            let tf_size = core::mem::size_of::<arch::riscv64::TrapFrame>();
            let tf_addr = kstack_top - tf_size;
            {
                let mut t = task.lock();
                t.trap_frame_ptr = tf_addr;
                // sp 指向 TrapFrame 下方，函数调用不会覆盖 TrapFrame
                t.context.sp = tf_addr - 8;
            }
            uart_puts("[boot] task A: PID=");
            uart_putchar(b'0' + task.lock().pid as u8);
            uart_putchar(b'\n');
            Some(task)
        }
        Err(_) => {
            uart_puts("[boot] WARN: ELF load failed for task A\n");
            None
        }
    };

    // --- 创建用户任务 B (user_loop.bin: 循环打印) ---
    let task_b = match arch::riscv64::load_elf_to_space(loop_elf) {
        Ok((user_space, entry, stack_top)) => {
            let root_ppn = user_space.root_ppn();
            let kstack_top = alloc_kernel_stack();
            let task = {
                let mut tm = TASK_MANAGER.get().unwrap().lock();
                tm.create_user_task(user_task_entry as usize, kstack_top, stack_top, root_ppn, entry)
            };
            let tf_size = core::mem::size_of::<arch::riscv64::TrapFrame>();
            let tf_addr = kstack_top - tf_size;
            {
                let mut t = task.lock();
                t.trap_frame_ptr = tf_addr;
                t.context.sp = tf_addr - 8;
            }
            uart_puts("[boot] task B: PID=");
            uart_putchar(b'0' + task.lock().pid as u8);
            uart_putchar(b'\n');
            Some(task)
        }
        Err(_) => {
            uart_puts("[boot] WARN: ELF load failed for task B\n");
            None
        }
    };

    // 将用户任务加入调度器
    {
        let mut sched = SCHEDULER.get().unwrap().lock();
        if let Some(ref task) = task_a { sched.enqueue(task.clone()); }
        if let Some(ref task) = task_b { sched.enqueue(task.clone()); }
    }
    uart_puts("[boot] user tasks enqueued\n");

    // ============================================================
    // Step 9.5: sret 切换到用户态（第一个用户任务）
    // ============================================================
    // 使用 per-task TrapFrame 启动用户任务。
    // 这样当任务被时钟中断打断时，寄存器会保存到正确的 TrapFrame，
    // 调度器可以安全地切换到其他任务。
    //
    // ## 教学概念：为什么不用 enter_user_mode？
    //
    // enter_user_mode 创建栈上的临时 TrapFrame，任务被抢占后
    // 寄存器保存到该临时 TrapFrame，但调度器不知道它的位置。
    // 使用 per-task TrapFrame（在 Task.trap_frame_ptr 中记录），
    // 调度器可以通过 NEXT_TRAP_FRAME 实现抢占式切换。
    if let Some(ref task) = task_a {
        let task = task.lock();
        CURRENT_PID.store(task.pid, Ordering::Relaxed);

        // 激活任务 A 的页表
        if task.page_table_root != 0 {
            // SAFETY: page_table_root 是有效的 SV39 页表根 PPN
            unsafe { arch::riscv64::switch_page_table(task.page_table_root); }
        }

        let tf_addr = task.trap_frame_ptr;
        // kernel_sp 位于 TrapFrame 下方（函数调用不会覆盖 TrapFrame）
        let kernel_sp = tf_addr - 8;
        // SAFETY: tf_addr 指向内核栈上分配的有效 TrapFrame
        unsafe {
            // 初始化 TrapFrame：设置用户态入口、栈指针、中断状态
            arch::riscv64::setup_user_trap_frame(
                &mut *(tf_addr as *mut arch::riscv64::TrapFrame),
                task.user_entry,
                task.ustack_top,
                kernel_sp,
                0,
            );
            // 将 TrapFrame 指针写入 sscratch（供 trap_entry 使用）
            core::arch::asm!(
                "csrw sscratch, {tf}",
                tf = in(reg) tf_addr,
                options(nomem, nostack)
            );
        }

        uart_puts("[boot] entering user mode with task A...\n");
        // 通过 trap_return 恢复 TrapFrame 并 sret 到用户态
        // SAFETY: TrapFrame 已正确初始化
        unsafe {
            arch::riscv64::trap_return(tf_addr as *mut arch::riscv64::TrapFrame);
        }
        // trap_return 不返回（已切换到用户态）
    }

    // ================================================================
    // Step 10: 启动调度器，进入 idle 循环
    // ================================================================
    // 从调度器取出第一个任务并执行
    // 当前只有一个 idle 任务，直接执行它
    //
    // ## 教学概念：调度器的工作方式
    //
    // 调度器维护一个就绪队列，每次 next() 返回下一个应该运行的任务。
    // Round-Robin 调度：所有任务轮流执行，每个任务运行一个时间片。
    // 当时钟中断到来时，当前任务被抢占，调度器选择下一个任务。
    //
    // 当前阶段：只有一个 idle 任务，所以总是调度它。
    // 后续添加更多任务后，调度器会在它们之间切换。
    uart_puts("suba: boot complete\n");
    uart_puts("[boot] starting scheduler...\n");

    if let Some(sched) = SCHEDULER.get() {
        let mut sched = sched.lock();
        if let Some(task) = sched.next() {
            let t = task.lock();
            uart_puts("[boot] scheduled PID ");
            uart_putchar(b'0' + t.pid as u8);
            uart_putchar(b'\n');
        }
    }

    // TODO(student): 实现真正的上下文切换
    // 需要 RISC-V Arch trait 实现后，调用：
    //   unsafe { ArchImpl::context_switch(current_ctx, next_ctx) };
    // 当前直接进入 idle 循环
    idle_loop();
}

// ===========================================================================
// idle 循环
// ===========================================================================

/// idle 循环 — CPU 空闲时执行此函数
///
/// ## 教学概念：idle 任务的作用
///
/// 当调度器中没有其他可运行任务时，CPU 执行 idle 循环。
/// idle 循环使用 `wfi` (Wait For Interrupt) 指令让 CPU 进入低功耗状态，
/// 直到有中断到来（如时钟中断）唤醒 CPU。
///
/// 这比忙等待（`loop {}`）更高效：
/// - 忙等待：CPU 持续执行空循环，消耗大量电能
/// - wfi：CPU 暂停执行，进入低功耗等待模式
///
/// 中断唤醒 CPU 后，中断处理函数执行（如调度决策），
/// 然后返回到 idle 循环继续等待。
fn idle_loop() -> ! {
    uart_puts("[boot] entering idle loop (wfi)\n");
    loop {
        // SAFETY: wfi 是 S-mode 合法指令，等待中断唤醒
        unsafe { asm!("wfi") }
    }
}

// ===========================================================================
// panic handler
// ===========================================================================

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // Phase 2 后续 feature 会添加 panic 输出
    power::shutdown(false);
}
