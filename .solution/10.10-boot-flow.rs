// Solution: 10.10 - 内核启动完整流程
//
// 本文件展示完整的内核启动流程。
// 完整代码位于 os/src/main.rs。

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
//   ├── Step 1: 打印启动横幅
//   │    uart_puts("[suba] booting on RISC-V...")
//   │
//   ├── Step 2: 初始化内核堆 (2MB)
//   │    heap::init_heap(heap_start, HEAP_SIZE)
//   │    GLOBAL_ALLOC.init(heap_start, HEAP_SIZE)
//   │
//   ├── Step 3: 初始化物理帧分配器
//   │    init_frame_allocator(frame_alloc_start, frame_alloc_end)
//   │    管理堆之后到物理内存末尾的区域
//   │
//   ├── Step 4: 创建内核页表并激活 SV39
//   │    init_kernel_page_table()
//   │    建立 VA=PA 的身份映射
//   │
//   ├── Step 5: 设置陷阱向量
//   │    init_trap()  →  csrw stvec, trap_entry
//   │
//   ├── Step 5.5: 初始化中断控制器
//   │    clint::init()  →  时钟中断
//   │    plic::init()   →  外部中断 (UART0)
//   │
//   ├── Step 6: 启用全局中断
//   │    enable_interrupts()  →  sstatus.SIE = 1
//   │
//   ├── Step 7: 页表测试 & 用户栈创建
//   │    run_tests()
//   │    UserAddrSpace::new() + map_user_stack()
//   │
//   ├── Step 8: 加载 init 程序
//   │    Riscv64ElfLoader::load_elf_to_space(elf_data)
//   │    user_space.activate()
//   │    enter_user_mode(entry, stack_top, kernel_sp, 0)
//   │    (后续 feature 11.3 完成)
//   │
//   ├── Step 9: 初始化任务系统
//   │    TaskManager::new()
//   │    RoundRobinScheduler::new()
//   │
//   ├── Step 10: 创建 idle 任务 (PID 1)
//   │    tm.create_task(idle_loop, boot_stack_top, 0)
//   │
//   └── Step 11: 启动调度
//        sched.next() → context_switch → idle_loop
//

#[unsafe(no_mangle)]
pub extern "C" fn rust_main() -> ! {
    // ---- Step 1: 打印启动信息 ----
    uart_puts("\n[suba] booting on RISC-V...\n");

    // ---- Step 2: 初始化内核堆 ----
    unsafe extern "C" { fn ekernel(); }
    let heap_start = ekernel as usize;
    unsafe { heap::init_heap(heap_start, heap::HEAP_SIZE); }
    unsafe { GLOBAL_ALLOC.init(heap_start, heap::HEAP_SIZE) };
    uart_puts("[suba] heap initialized\n");

    // ---- Step 3: 初始化物理帧分配器 ----
    let frame_alloc_start = heap_start + heap::HEAP_SIZE;
    let frame_alloc_end = suba_kernel::mm::address::PHYS_MEMORY_START
        + suba_kernel::mm::address::PHYS_MEMORY_SIZE;
    unsafe { arch::riscv64::page::init_frame_allocator(frame_alloc_start, frame_alloc_end); }
    uart_puts("[suba] frame allocator initialized\n");

    // ---- Step 4: 初始化内核页表 ----
    arch::riscv64::init_kernel_page_table();
    uart_puts("[suba] kernel page table activated (SV39)\n");

    // ---- Step 5: 设置陷阱向量 ----
    arch::riscv64::init_trap();
    uart_puts("[suba] trap vector set\n");

    // ---- Step 5.5: 初始化中断控制器 ----
    driver::clint::init();
    uart_puts("[suba] CLINT initialized (timer interrupt)\n");
    driver::plic::init();
    uart_puts("[suba] PLIC initialized (UART0 → S-mode)\n");

    // ---- Step 6: 启用全局中断 ----
    arch::riscv64::Riscv64CpuOps::enable_interrupts();
    uart_puts("[suba] interrupts enabled (sstatus.SIE=1)\n");

    // ---- Step 7: 页表测试 & 用户栈 ----
    arch::riscv64::page::run_tests();
    // (用户栈创建测试省略，见实际代码)

    // ---- Step 8: 加载 init 程序 ----
    // TODO: 嵌入 init ELF 并加载到用户地址空间
    // let elf_data = include_bytes!("../../user/init.bin");
    // let (entry, stack_top, user_space) = Riscv64ElfLoader::load_elf_to_space(elf_data)?;
    // unsafe { user_space.activate(); }
    // enter_user_mode(entry, stack_top, kernel_sp, 0);

    // ---- Step 9: 初始化任务系统 ----
    let mut tm = TaskManager::new();
    let mut sched = RoundRobinScheduler::new();

    // ---- Step 10: 创建 idle 任务 ----
    unsafe extern "C" { fn boot_stack_top(); }
    let idle_task = tm.create_task(idle_loop as usize, boot_stack_top as usize, 0);
    sched.enqueue(idle_task);

    // ---- Step 11: 启动调度 ----
    if let Some(task) = sched.next() {
        let _t = task.lock();
        // context_switch 到 idle_task
    }
    idle_loop();
}
