// Solution: 11.2 - 创建第一个任务并调度
//
// 本文件展示如何创建第一个任务并通过上下文切换执行。
// 完整代码位于 os/src/main.rs。

// ============================================================================
// 任务创建与调度
// ============================================================================

// 在 rust_main 中，初始化任务系统后：
//
//   // 创建 idle 任务 (PID 1)
//   let idle_task = tm.create_task(
//       idle_loop as usize,      // 入口函数
//       boot_stack_top as usize, // 内核栈顶
//   );
//   sched.enqueue(idle_task);
//
//   // 取出下一个任务并切换
//   if let Some(task) = sched.next() {
//       let t = task.lock();
//       let next_ctx = &t.context as *const Context;
//
//       // 创建一个"当前上下文"用于保存
//       // 第一次切换时，当前上下文是 rust_main 的栈帧
//       let mut current_ctx = Context::zero_init();
//
//       // 上下文切换：保存当前到 current_ctx，恢复 idle_task
//       // SAFETY: next_ctx 指向有效的任务上下文
//       unsafe {
//           Riscv64Arch::context_switch(
//               &mut current_ctx as *mut Context,
//               next_ctx,
//           );
//       }
//       // 切换后，CPU 开始执行 idle_task 的入口 (idle_loop)
//       // 当 idle_task 让出 CPU 时，会切换回这里
//   }
//
//   // 如果没有任务可调度，直接进入 idle
//   idle_loop();

// ============================================================================
// 教学概念：第一次上下文切换
// ============================================================================

// 第一次上下文切换的特殊之处：
// 1. 没有"前一个任务"——当前执行流是 rust_main，不是任务
// 2. 需要创建一个"假的"当前上下文来保存 rust_main 的状态
// 3. switch.S 的 context_switch 会：
//    - 保存 rust_main 的 callee-saved 寄存器到 current_ctx
//    - 从 idle_task 的 context 恢复寄存器
//    - 跳转到 idle_task 的 ra（即 idle_loop）
//
// 后续切换就正常了：从任务 A 切换到任务 B，
// 保存 A 的寄存器，恢复 B 的寄存器。

// ============================================================================
// idle_loop — idle 任务的入口
// ============================================================================

fn idle_loop() -> ! {
    uart_puts("[boot] entering idle loop\n");
    loop {
        // SAFETY: wfi 是特权指令，等待中断唤醒
        unsafe { core::arch::asm!("wfi") }
    }
}
