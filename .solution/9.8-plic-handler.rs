// .solution/9.8-plic-handler.rs — PLIC 外部中断处理参考实现
//
// 本文件是 feature 9.8 的参考实现。
// 学生应在 os/src/driver/plic.rs 中实现外部中断处理。
//
// ## 实现要点
//
// 1. claim() — 读取 Claim 寄存器获取中断源 ID
// 2. complete() — 写入 Complete 寄存器通知 PLIC
// 3. handle_external_interrupt() — 中断分发（claim → dispatch → complete）
// 4. init() 中使能 sie.SEIE（Supervisor External Interrupt Enable）
//
// ## 教学概念：Claim-Complete 模式
//
// PLIC 使用 Claim-Complete 而非传统的 ACK-EOI：
// - Claim（读取）：原子地获取最高优先级挂起中断，PLIC 自动清除 Pending
// - Complete（写入）：通知 PLIC 中断处理完毕
//
// ## sie.SEIE
//
// 即使 PLIC 配置正确，sie.SEIE = 0 会阻止外部中断到达 S-mode。
// init() 中必须设置 sie.SEIE (bit 9)。
//
// init() 中的 sie.SEIE 设置：
// unsafe { core::arch::asm!("csrs sie, {0}", in(reg) 1 << 9); }
