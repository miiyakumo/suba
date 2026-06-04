// Solution: 11.6 - 用户态 syscall exit 路径
//
// 本文件展示 exit 系统调用的完整路径实现。
// 完整代码位于 os/src/main.rs 和 os/src/arch/riscv64/mod.rs。
//
// ============================================================================
// exit 路径总览
// ============================================================================
//
// 用户程序                     内核
// ┌──────────┐               ┌──────────────────────┐
// │ li a7, 93│   ecall       │ trap_entry (asm)      │
// │ li a0, 0 │ ────────────► │   保存全部寄存器       │
// │ ecall    │               │ trap_handler()        │
// │          │               │   scause == 8         │
// │ (终止)    │               │   sepc += 4           │
// └──────────┘               │   dispatch(tf)        │
//                            │   → sys_exit(0)       │
//                            │   after_syscall(tf)   │
//                            │   → 检查 a7 == 93     │
//                            │   → 标记 Exited       │
//                            │   → schedule()        │
//                            │   → 关机或切换任务     │
//                            └──────────────────────┘
//
// ============================================================================
// 关键实现细节
// ============================================================================
//
// 1. 全局任务管理器和调度器
//
//    static TASK_MANAGER: spin::Once<spin::Mutex<TaskManager>>;
//    static SCHEDULER: spin::Once<spin::Mutex<RoundRobinScheduler>>;
//    static CURRENT_PID: AtomicUsize;
//
//    trap_handler 在中断上下文中执行，无法通过参数获取任务管理器。
//    使用全局静态变量是内核中常见的做法。
//
// 2. 回调模式
//
//    static mut AFTER_SYSCALL: Option<fn(&mut TrapFrame)> = None;
//
//    内核核心（kernel crate）不直接依赖硬件后端（os crate）。
//    通过回调模式，硬件后端注册自己的处理逻辑。
//    这体现了操作系统的分层架构。
//
// 3. after_syscall_exit 函数
//
//    fn after_syscall_exit(tf: &mut TrapFrame) {
//        // 检查是否是 exit 系统调用
//        if tf.x17_a7 != SYS_EXIT { return; }
//
//        // 获取退出码
//        let exit_code = tf.x10_a0 as i32;
//
//        // 标记当前任务为 Exited
//        let pid = CURRENT_PID.load(Ordering::Relaxed);
//        if let Some(tm) = TASK_MANAGER.get() {
//            let tm = tm.lock();
//            if let Some(task) = tm.get_task(pid) {
//                let mut task = task.lock();
//                task.state = TaskState::Exited;
//                task.exit_code = exit_code;
//            }
//        }
//
//        // 调度下一个任务
//        schedule();
//    }
//
// 4. schedule 函数
//
//    fn schedule() -> ! {
//        if let Some(sched) = SCHEDULER.get() {
//            let mut sched = sched.lock();
//            if let Some(next_task) = sched.next() {
//                let mut task = next_task.lock();
//                task.state = TaskState::Running;
//                // ... 上下文切换
//            }
//        }
//        // 没有就绪任务，关机
//        power::shutdown(false)
//    }
//
// ============================================================================
// trap_handler 中的钩子
// ============================================================================
//
// 在 trap_handler 的 U-mode ecall 处理中，dispatch 返回后调用回调：
//
//    8 => {
//        tf.sepc = tf.sepc.wrapping_add(4);
//        suba_kernel::syscall::dispatch(tf);
//
//        // 系统调用后处理回调
//        unsafe {
//            if let Some(callback) = AFTER_SYSCALL {
//                callback(tf);
//            }
//        }
//    }
//
// ============================================================================
// 教学要点
// ============================================================================
//
// 1. exit 不会返回到用户程序 — 任务被标记为 Exited，调度器选择下一个任务
// 2. 全局 CURRENT_PID 用于在 trap 处理中识别当前任务
// 3. 回调模式保持了 kernel crate 和 os crate 的分层架构
// 4. schedule() 是 noreturn 函数 — 它要么切换到另一个任务，要么关机
