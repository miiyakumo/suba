// Solution: 11.1 - 内核启动并打印信息
//
// 本文件展示内核启动验证的关键代码。
// 完整代码位于 os/src/main.rs。

// ============================================================================
// 启动验证
// ============================================================================

// 在 rust_main 的启动流程末尾，打印 "boot complete" 标记：
//
//   uart_puts("suba: boot complete\n");
//
// QEMU 运行时，通过串口输出验证启动成功：
//   $ qemu-system-riscv64 -machine virt -nographic -kernel os/target/.../os
//   ========================================
//     suba kernel — RISC-V 64-bit
//   ========================================
//   [boot] heap initialized
//   [boot] frame allocator initialized
//   [boot] kernel page table activated (SV39)
//   [boot] trap vector set
//   [boot] CLINT initialized (timer interrupt)
//   [boot] PLIC initialized (UART0 → S-mode)
//   [boot] interrupts enabled (sstatus.SIE=1)
//   [boot] task system ready (idle task PID=1)
//   [boot] init: skipped (no embedded ELF yet)
//   suba: boot complete
//   [boot] starting scheduler...
//   [boot] scheduled PID 1
//   [boot] entering idle loop

// 验证要点：
// 1. 所有初始化步骤无 panic
// 2. "suba: boot complete" 出现在输出中
// 3. 调度器成功调度 idle 任务
// 4. 进入 idle 循环（wfi）
