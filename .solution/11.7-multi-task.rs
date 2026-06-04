// Solution: 11.7 — 多任务调度验证
//
// 本文件展示多任务调度的完整实现。
//
// ============================================================================
// 核心架构
// ============================================================================
//
// 1. 每个用户任务有独立的 SV39 页表（page_table_root 字段）
// 2. 调度器通过 context_switch 切换任务
// 3. 用户任务通过 trampoline (user_task_entry) 进入用户态
// 4. trampoline 激活页表后 sret 到用户程序
//
// ============================================================================
// 关键数据结构
// ============================================================================
//
// Task 结构体新增字段：
//   - page_table_root: usize  — 页表根 PPN（0 = 内核任务）
//   - user_entry: usize       — 用户程序入口地址
//
// TaskManager 新增方法：
//   - create_user_task(entry, kstack, ustack, ppn, user_entry) -> TaskHandle
//
// ============================================================================
// 多任务创建流程
// ============================================================================
//
// 1. 加载 ELF → (UserAddrSpace, entry, stack_top)
// 2. 获取 root_ppn = user_space.root_ppn()
// 3. 创建用户任务：create_user_task(trampoline, kstack, ustack, ppn, entry)
// 4. 将任务加入调度器队列
// 5. 第一个任务通过 enter_user_mode 直接进入用户态
//
// ============================================================================
// 调度流程（exit 路径）
// ============================================================================
//
// 用户程序 ecall(SYS_EXIT)
//   → trap_entry → trap_handler → dispatch → sys_exit
//   → after_syscall_exit: 标记 Exited
//   → schedule():
//     → sched.next() 获取下一个就绪任务
//     → context_switch(dummy, next_task.context)
//     → 如果是用户任务：user_task_entry → activate page table → sret
//     → 如果是内核任务：直接跳转到 task.context.ra (idle_loop)
//
// ============================================================================
// trampoline 函数
// ============================================================================
//
// fn user_task_entry() -> ! {
//     let pid = CURRENT_PID.load();
//     let task = TASK_MANAGER.get_task(pid);
//     switch_page_table(task.page_table_root);
//     drop_to_user_mode(task.user_entry, task.ustack_top, task.kstack_top, 0);
// }
//
// ============================================================================
// 教学要点
// ============================================================================
//
// 1. 每个用户任务需要独立的页表来实现地址空间隔离
// 2. context_switch 只能切换到 S-mode 代码，用户态需要 trampoline
// 3. trampoline 激活页表后通过 sret 切换到用户态
// 4. Context 结构体在 Task::new 中初始化（ra=entry, sp=kstack_top）
// 5. 调度器只关心 TaskState，不关心页表或地址空间
