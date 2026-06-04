// Solution: 11.8 - 时钟中断驱动调度
//
// 本文件展示时钟中断驱动任务调度的完整实现路径：
// CLINT 产生中断 → trap_handler → 更新 tick → 触发 schedule()
// → context_switch → NEXT_TRAP_FRAME → 任务切换。
//
// 完整代码分布在以下文件中：
//   os/src/driver/clint.rs   — CLINT 初始化和中断处理
//   os/src/arch/riscv64/trap.S — 陷阱入口/恢复（NEXT_TRAP_FRAME 检查）
//   os/src/arch/riscv64/mod.rs — trap_handler 分发
//   os/src/main.rs            — schedule() 和 schedule_fn()

// ============================================================================
// Part 1: CLINT 时钟初始化 (os/src/driver/clint.rs)
// ============================================================================

/// 初始化时钟中断
///
/// 设置第一次时钟中断，并使能 sie.STIE 位。
/// 在 rust_main() 的 Step 6 调用。
pub fn init() {
    // 设置第一次时钟中断（mtimecmp = mtime + TIMER_INTERVAL）
    set_next_timer();

    // 使能 Supervisor timer interrupt
    // sie (Supervisor Interrupt Enable) 寄存器的 bit 5 是 STIE
    // SAFETY: 设置 sie.STIE 位，允许时钟中断传递到 S-mode
    unsafe {
        core::arch::asm!(
            "csrs sie, {0}",
            in(reg) 1 << 5,
            options(nomem, nostack)
        );
    }
}

/// 设置下一次时钟中断
///
/// 通过 SBI 调用 `set_timer` 设置 mtimecmp 寄存器。
/// 当 mtime >= mtimecmp 时，触发 Supervisor timer interrupt (scause = 5)。
///
/// ## 教学概念：SBI 时钟接口
///
/// S-mode 无法直接写入 mtimecmp（M-mode 寄存器）。
/// SBI 的 Timer Extension 提供了 `set_timer` 调用，
/// OpenSBI 收到后会写入当前 hart 的 mtimecmp 寄存器。
pub fn set_next_timer() {
    let current = get_time();
    sbi_rt::set_timer(current + TIMER_INTERVAL);
}

/// 时钟中断间隔（mtime tick 数）
///
/// QEMU mtime 频率约 10 MHz，1_000_000 tick ≈ 100ms。
pub const TIMER_INTERVAL: u64 = 1_000_000;

/// 时间片大小（tick 数）
///
/// 每 TIME_SLICE_TICKS 个时钟中断触发一次调度。
pub const TIME_SLICE_TICKS: usize = 2;

// ============================================================================
// Part 2: 时钟中断处理 (os/src/driver/clint.rs)
// ============================================================================

/// 处理时钟中断 — 由 trap_handler 在 scause == 5 时调用
///
/// ## 教学概念：时钟中断处理流程
///
/// ```text
/// 硬件: mtime >= mtimecmp
///   → CPU 触发 Supervisor timer interrupt (scause = 5)
///   → trap_handler 分发到此处
///   → 1. 递增 tick 计数
///   → 2. 设置下一次中断（维持周期性）
///   → 3. 检查时间片是否用完
///   → 4. 如果用完且来自用户态，触发调度
/// ```
///
/// ## 教学概念：时间片轮转调度
///
/// 每个任务运行 TIME_SLICE_TICKS 个 tick 后被抢占：
/// 1. 时钟中断递增 tick 计数
/// 2. 每 TIME_SLICE_TICKS 个 tick 重置计数器并触发调度
/// 3. 调度器选择下一个就绪任务，通过 context_switch 切换
///
/// 这实现了"公平"的 CPU 时间分配：没有任务能独占 CPU。
///
/// ## 教学概念：为什么只抢占用户态任务？
///
/// SPP (Supervisor Previous Privilege) 指示中断发生时的特权级：
/// - SPP=0 (User): 来自用户态，可以安全抢占
/// - SPP=1 (Supervisor): 来自内核态（可能正在持有锁或执行关键操作）
///
/// 抢占内核态任务可能导致：
/// - 死锁：被抢占的任务持有锁，调度器上的其他任务等待同一把锁
/// - 数据损坏：关键区操作未完成就被打断
///
/// TODO(student): CLINT 时钟中断 → 调度验证
/// 验证时钟中断触发调度的完整路径：
/// 1. CLINT 产生时钟中断 (scause=5)
/// 2. trap_handler 分发到 handle_timer_interrupt
/// 3. 递增 tick 计数
/// 4. 检查时间片是否用完
/// 5. SPP=0 (来自用户态) 时触发 schedule()
/// 6. context_switch 切换到新任务
/// 7. NEXT_TRAP_FRAME 通知 trap.S 恢复新任务
/// 8. sret 切换到新任务的用户态
pub fn handle_timer_interrupt() {
    // 递增全局 tick 计数
    TIMER_TICKS.fetch_add(1, Ordering::Relaxed);

    // 设置下一次时钟中断（保持周期性）
    set_next_timer();

    // 时间片调度检查
    // 每 TIME_SLICE_TICKS 个 tick 检查一次是否需要调度
    let ticks = TIMER_TICKS.load(Ordering::Relaxed);
    if ticks % TIME_SLICE_TICKS == 0 {
        // 读取 sstatus 检查是否来自用户态
        // 只有用户态任务才需要被抢占（内核态任务正在处理系统调用）
        let sstatus: usize;
        // SAFETY: sstatus 是只读 CSR
        unsafe {
            core::arch::asm!(
                "csrr {}, sstatus",
                out(reg) sstatus,
                options(nomem, nostack)
            );
        }
        let spp = (sstatus >> 8) & 1; // SPP bit (bit 8)
        if spp == 0 {
            // 来自用户态：触发调度决策
            // schedule_fn() 会设置 NEXT_TRAP_FRAME，trap.S 在 trap_handler 返回后检查
            // SAFETY: SCHEDULE_FN 在 init_schedule_fn 中初始化
            unsafe {
                if let Some(schedule_fn) = SCHEDULE_FN {
                    let next_tf = schedule_fn();
                    // schedule_fn 内部设置了 NEXT_TRAP_FRAME
                    // 返回值是下一个任务的 TrapFrame 指针
                    if !next_tf.is_null() {
                        crate::arch::riscv64::NEXT_TRAP_FRAME.store(
                            next_tf,
                            Ordering::Release,
                        );
                    }
                }
            }
        }
    }
}

// ============================================================================
// Part 3: 调度回调注册 (os/src/main.rs)
// ============================================================================

/// 调度函数 — 由 clint.rs 的时钟中断处理调用
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

/// 注册调度函数（由 clint.rs 时钟中断调用）
///
/// 在 rust_main 中调用，将调度函数注册到 CLINT 模块。
/// 当时钟中断来自用户态且时间片用完时，clint 调用此函数触发任务切换。
///
/// # Safety
/// 必须在中断处理之前调用（单线程初始化阶段）。
unsafe fn register_scheduler() {
    unsafe {
        crate::driver::clint::init_schedule_fn(schedule_fn);
    }
}

// ============================================================================
// Part 4: 调度器实现 (os/src/main.rs)
// ============================================================================

/// 调度器 — 选择下一个任务并切换
///
/// 从调度器取出下一个就绪任务，通过上下文切换跳转到该任务。
/// 如果没有就绪任务，进入 idle 循环（等待中断唤醒）。
///
/// ## 教学概念：抢占式调度的完整流程
///
/// ```text
/// Task A (用户态)
///   │ 时钟中断
///   ▼
/// trap_entry
///   │ 保存全部寄存器到 Task A 的 TrapFrame
///   ▼
/// trap_handler
///   │ scause = 5 (timer interrupt)
///   ▼
/// handle_timer_interrupt()
///   │ ticks++, SPP=0 → call schedule_fn()
///   ▼
/// schedule_fn() → schedule()
///   │ 1. 从 RoundRobinScheduler 取出 Task B
///   │ 2. 设置 CURRENT_PID = Task B.pid
///   │ 3. 切换到 Task B 的页表
///   │ 4. 设置 NEXT_TRAP_FRAME = Task B.trap_frame_ptr
///   │ 5. context_switch(Task A.ctx, Task B.ctx)
///   ▼
/// context_switch (switch.S)
///   │ 保存 callee-saved 到 Task A.ctx
///   │ 从 Task B.ctx 恢复 callee-saved
///   │ ret → Task B.ctx.ra
///   ▼
/// Task B 运行
///   │ 首次: user_task_entry → trap_return → sret → 用户态
///   │ 恢复: 返回到 schedule() 内部
///   │       → schedule_fn() → handle_timer_interrupt()
///   │       → trap_handler() → trap.S
///   │       → 检查 NEXT_TRAP_FRAME
///   │       → 非 null: 从新 TrapFrame 恢复 → sret
///   │       → null: 从当前 TrapFrame 恢复 → sret
/// ```
fn schedule() {
    // 从调度器获取下一个就绪任务
    let next_info = if let Some(sched) = SCHEDULER.get() {
        sched.lock().next().and_then(|task| {
            let mut t = task.lock();
            t.state = TaskState::Running;
            let pid = t.pid;
            CURRENT_PID.store(pid, Ordering::Relaxed);
            Some((t.context_ptr(), t.trap_frame_ptr, t.page_table_root))
        })
    } else {
        None
    };

    if let Some((next_ctx, trap_frame_ptr, page_table_root)) = next_info {
        // 切换到新任务的页表
        if page_table_root != 0 {
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
    }
}

// ============================================================================
// Part 5: trap_handler 分发 (os/src/arch/riscv64/mod.rs)
// ============================================================================

/// trap_handler — 陷阱分发函数
///
/// 当 scause = (1<<63)|5（时钟中断）时，调用 clint::handle_timer_interrupt()
/// 处理时钟中断，触发调度决策。
pub unsafe extern "C" fn trap_handler(trap_frame: *mut TrapFrame) {
    let tf = unsafe { &mut *trap_frame };
    let scause = read_scause();
    const INTERRUPT_BIT: usize = 1 << (usize::BITS - 1);

    if scause & INTERRUPT_BIT != 0 {
        // 中断处理
        match scause & !INTERRUPT_BIT {
            5 => {
                // Supervisor timer interrupt (STI)
                // CLINT: mtime >= mtimecmp
                // → 递增 tick、设置下一次中断、检查调度
                crate::driver::clint::handle_timer_interrupt();
            }
            // ... 其他中断类型
            _ => {}
        }
    }
    // trap_handler 返回后，trap.S 检查 NEXT_TRAP_FRAME
}

// ============================================================================
// Part 6: trap.S — NEXT_TRAP_FRAME 检查 (os/src/arch/riscv64/trap.S)
// ============================================================================
//
// trap_entry 在调用 trap_handler 之后检查 NEXT_TRAP_FRAME：
//
// ```asm
// call trap_handler
//
// # 检查 NEXT_TRAP_FRAME（抢占式调度）
// la t0, NEXT_TRAP_FRAME
// ld a0, 0(t0)
// beqz a0, .Lreturn_to_kernel
//
// # NEXT_TRAP_FRAME 非空：从新任务的 TrapFrame 恢复
// sd zero, 0(t0)    # 清空 NEXT_TRAP_FRAME（一次性使用）
//
// # a0 = 新任务的 TrapFrame 指针
// # fall through to trap_return
// ```
//
// trap_return 从新 TrapFrame 恢复所有寄存器并 sret 到用户态。

// ============================================================================
// 教学概念总结：时钟中断驱动调度的完整路径
// ============================================================================
//
// ```text
//                         硬件层                   内核层
// ┌─────────────────────────────────────────────────────────────────────┐
// │                                                                     │
// │  CLINT (硬件)                                                        │
// │  mtime >= mtimecmp                                                  │
// │    │                                                                │
// │    ▼                                                                │
// │  CPU 触发中断                                                        │
// │  sepc←PC, scause←5, sstatus.SPP←0                                   │
// │  PC←stvec (trap_entry)                                              │
// │    │                                                                │
// │    ▼                                                                │
// │  trap_entry (trap.S)                                                │
// │  保存全部寄存器到 TrapFrame                                           │
// │  切换到内核栈                                                        │
// │    │                                                                │
// │    ▼                                                                │
// │  trap_handler (mod.rs)                                              │
// │  scause=5 → handle_timer_interrupt()                                │
// │    │                                                                │
// │    ▼                                                                │
// │  handle_timer_interrupt (clint.rs)                                  │
// │  1. ticks++                                                         │
// │  2. set_next_timer()                                                │
// │  3. 检查时间片                                                       │
// │  4. SPP=0 → schedule_fn()                                          │
// │    │                                                                │
// │    ▼                                                                │
// │  schedule_fn → schedule (main.rs)                                   │
// │  1. RoundRobinScheduler.next() → Task B                            │
// │  2. 切换页表 (Task B 的用户页表)                                     │
// │  3. NEXT_TRAP_FRAME ← Task B.trap_frame_ptr                        │
// │  4. context_switch(Task A.ctx, Task B.ctx)                         │
// │    │                                                                │
// │    ▼                                                                │
// │  context_switch (switch.S)                                          │
// │  保存 callee-saved 到 Task A.ctx                                    │
// │  从 Task B.ctx 恢复 callee-saved                                    │
// │  ret → Task B.ctx.ra                                               │
// │    │                                                                │
// │    ▼                                                                │
// │  Task B 运行                                                         │
// │  首次: user_task_entry → trap_return → sret → 用户态                │
// │  恢复: 返回 schedule() → 回溯到 trap.S                              │
// │    │                                                                │
// │    ▼                                                                │
// │  trap.S 检查 NEXT_TRAP_FRAME                                        │
// │  非 null → trap_return(新 TrapFrame) → sret → 新任务用户态          │
// │  null → 恢复原 TrapFrame → sret → 原任务用户态                       │
// │                                                                     │
// └─────────────────────────────────────────────────────────────────────┘
// ```
//
// ## 关键设计决策
//
// 1. **为什么用 NEXT_TRAP_FRAME 而不是直接恢复？**
//    trap_handler 执行期间，寄存器状态已被 Rust 函数调用破坏。
//    只有 trap.S 汇编能完整恢复用户态寄存器。
//    NEXT_TRAP_FRAME 是 trap_handler 与 trap.S 之间的通信通道。
//
// 2. **为什么只抢占用户态任务？**
//    内核态任务可能持有锁或处于关键区，抢占会导致死锁或数据损坏。
//    检查 sstatus.SPP 确保只在安全时抢占。
//
// 3. **为什么 context_switch 只保存 callee-saved 寄存器？**
//    调用者保存的寄存器（t0-t6, a0-a7）已在函数调用中自然保存。
//    context_switch 只需要保存那些"跨函数调用存活"的寄存器（s0-s11, ra, sp）。
//
// 4. **为什么每个任务需要独立的内核栈？**
//    当时钟中断打断任务 A，切换到任务 B，任务 B 又被中断...
//    每个任务的内核栈独立，确保嵌套中断不会覆盖数据。
