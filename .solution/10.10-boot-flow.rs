// Solution: 10.10 - 内核启动完整流程
//
// 本文件展示完整的内核启动流程。
// 完整代码位于 os/src/main.rs。
//
// ============================================================================
// rust_main — 完整内核启动流程
// ============================================================================
//
// 启动流程（按顺序）：
//
//   entry.S (_start)
//   ├── 设置栈指针 (sp → boot_stack_top)
//   ├── 清零 BSS 段
//   └── 跳转 rust_main
//
//   rust_main()
//   ├── Step 1:  打印启动横幅
//   │    uart_puts("suba kernel — RISC-V 64-bit")
//   │
//   ├── Step 2:  初始化内核堆 (2MB)
//   │    heap::init_heap(heap_start, HEAP_SIZE)
//   │    GLOBAL_ALLOC.init(heap_start, HEAP_SIZE)
//   │
//   ├── Step 3:  初始化物理帧分配器
//   │    init_frame_allocator(frame_alloc_start, frame_alloc_end)
//   │    管理堆之后到物理内存末尾的区域
//   │
//   ├── Step 4:  创建内核页表并激活 SV39
//   │    init_kernel_page_table()
//   │    建立 VA=PA 的身份映射（1GB 大页 × 4）
//   │
//   ├── Step 5:  设置陷阱向量
//   │    init_trap()  →  csrw stvec, trap_entry
//   │
//   ├── Step 6:  初始化中断控制器
//   │    clint::init()  →  时钟中断 (sie.STIE)
//   │    plic::init()   →  外部中断 (sie.SEIE, UART0)
//   │
//   ├── Step 7:  启用全局中断
//   │    enable_interrupts()  →  sstatus.SIE = 1
//   │    两级控制：sie (类型级) + sstatus.SIE (全局级)
//   │
//   ├── Step 8:  初始化任务系统
//   │    TaskManager::new()
//   │    RoundRobinScheduler::new()
//   │    创建 idle 任务 (PID=1)，入队
//   │
//   ├── Step 9:  加载 init 程序
//   │    (后续 feature 完成)
//   │    Riscv64ElfLoader::load_elf_to_space(elf_data)
//   │    user_space.activate()
//   │    enter_user_mode(entry, stack_top, kernel_sp, 0)
//   │
//   └── Step 10: 启动调度器
//        sched.next() → idle_loop (wfi 等待中断)
//
// ============================================================================
// 关键设计决策
// ============================================================================
//
// 1. 中断在 Step 7 才启用（所有子系统初始化完毕后）
//    → 避免中断处理函数访问未初始化的数据结构
//
// 2. 任务系统在 Step 8 初始化（堆和中断就绪后）
//    → 任务结构体需要堆分配，调度需要时钟中断驱动
//
// 3. idle 任务使用 boot_stack（不需要额外分配栈）
//    → idle 任务是内核线程，使用内核栈
//
// 4. init 程序加载在 Step 9（任务系统就绪后）
//    → 用户程序需要用户地址空间（页表 + 帧分配器）
//    → 用户任务需要任务系统管理
//

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // Step 1: 打印启动横幅
    uart_puts("\n========================================\n");
    uart_puts("  suba kernel — RISC-V 64-bit\n");
    uart_puts("========================================\n");

    // Step 2: 初始化内核堆
    unsafe extern "C" { fn ekernel(); }
    let heap_start = ekernel as usize;
    unsafe { heap::init_heap(heap_start, heap::HEAP_SIZE); }
    unsafe { GLOBAL_ALLOC.init(heap_start, heap::HEAP_SIZE) };
    uart_puts("[boot] heap initialized (2MB)\n");

    // Step 3: 初始化物理帧分配器
    let frame_alloc_start = heap_start + heap::HEAP_SIZE;
    let frame_alloc_end = suba_kernel::mm::address::PHYS_MEMORY_START
        + suba_kernel::mm::address::PHYS_MEMORY_SIZE;
    unsafe { arch::riscv64::page::init_frame_allocator(frame_alloc_start, frame_alloc_end); }
    uart_puts("[boot] frame allocator initialized\n");

    // Step 4: 创建内核页表并激活 SV39
    arch::riscv64::init_kernel_page_table();
    uart_puts("[boot] kernel page table activated (SV39)\n");

    // Step 5: 设置陷阱向量
    arch::riscv64::init_trap();
    uart_puts("[boot] trap vector set (stvec → trap_entry)\n");

    // Step 6: 初始化中断控制器
    driver::clint::init();
    uart_puts("[boot] CLINT initialized (timer interrupt)\n");
    driver::plic::init();
    uart_puts("[boot] PLIC initialized (UART0 → S-mode)\n");

    // Step 7: 启用全局中断
    use suba_kernel::arch::CpuOps;
    arch::riscv64::Riscv64CpuOps::enable_interrupts();
    uart_puts("[boot] interrupts enabled (sstatus.SIE=1)\n");

    // Step 8: 初始化任务系统
    let mut tm = TaskManager::new();
    let mut sched = RoundRobinScheduler::new();
    unsafe extern "C" { fn boot_stack_top(); }
    let idle_task = tm.create_task(idle_loop as usize, boot_stack_top as usize, 0);
    sched.enqueue(idle_task);
    uart_puts("[boot] task system ready (idle task PID=1)\n");

    // Step 9: 加载 init 程序
    // TODO: 从 initrd 获取 ELF 数据
    // let elf_data = include_bytes!("../../user/init.bin");
    // let (user_space, entry, stack_top) = load_elf_to_space(elf_data).unwrap();
    // unsafe { user_space.activate(); }
    // enter_user_mode(entry, stack_top, kernel_sp, 0);
    uart_puts("[boot] init: skipped (no embedded ELF yet)\n");

    // Step 10: 启动调度器
    uart_puts("[boot] starting scheduler...\n");
    if let Some(task) = sched.next() {
        let t = task.lock();
        uart_puts("[boot] scheduled PID ");
        uart_putchar(b'0' + t.pid as u8);
        uart_putchar(b'\n');
    }
    idle_loop();
}

fn idle_loop() -> ! {
    uart_puts("[boot] entering idle loop (wfi)\n");
    loop {
        // SAFETY: wfi 是 S-mode 合法指令，等待中断唤醒
        unsafe { core::arch::asm!("wfi") }
    }
}
