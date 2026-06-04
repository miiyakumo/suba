// .solution/9.9-enable-interrupts.rs — 启用中断和优先级控制参考实现
//
// 本文件是 feature 9.9 的参考实现。
// 学生应在 os/src/main.rs 中配置中断使能。
//
// ## 实现要点
//
// 1. 调用 clint::init() 初始化时钟中断（设置 sie.STIE）
// 2. 调用 plic::init() 初始化外部中断（设置 sie.SEIE）
// 3. 调用 enable_interrupts() 设置 sstatus.SIE = 1
//
// ## 教学概念：中断使能的两级控制
//
// RISC-V 中断使能有两级：
//
// ```text
// 第一级：sie 寄存器（按中断类型使能）
//   sie.STIE (bit 5)  — Supervisor Timer Interrupt Enable
//   sie.SEIE (bit 9)  — Supervisor External Interrupt Enable
//   sie.SSIE (bit 1)  — Supervisor Software Interrupt Enable
//
// 第二级：sstatus.SIE（全局总开关）
//   SIE = 1: 允许 CPU 响应已使能的中断
//   SIE = 0: 忽略所有 S-mode 中断
// ```
//
// 两级都打开，中断才能到达 CPU。
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
//
// ## 参考代码（os/src/main.rs 中添加）
//
// // 初始化中断控制器
// driver::clint::init();  // 设置 sie.STIE
// driver::plic::init();   // 设置 sie.SEIE
//
// // 启用全局中断
// arch::riscv64::Riscv64CpuOps::enable_interrupts();  // 设置 sstatus.SIE = 1
//
// 注意：必须在中断控制器初始化之后再启用全局中断，
// 否则可能出现中断已到达但处理函数未就绪的情况。
