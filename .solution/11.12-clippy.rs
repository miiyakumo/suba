// Solution: 11.12 - Clippy 和质量检查
//
// 本文件记录最终质量验收的检查项和修复。
//
// 质量检查完成后状态：
// - cargo clippy (kernel): 0 warnings
// - cargo clippy (os): 0 warnings
// - cargo test (kernel): 92 passed, 0 failed
// - cargo build (os, riscv64): 成功, 零警告

// ============================================================================
// 修复的 Clippy 警告清单
// ============================================================================

// --- os crate (7 警告已修复) ---

// 1. power.rs: empty_line_after_doc_comment
//    修复：移除模块注释末尾多余空行

// 2. debug_console.rs: empty_line_after_doc_comment
//    修复：移除模块注释末尾多余空行

// 3. arch/riscv64/mod.rs: doc_lazy_continuation
//    修复：在编号列表和段落之间添加空行

// 4. driver/clint.rs: manual_is_multiple_of
//    修复：ticks % TIME_SLICE_TICKS == 0 → ticks.is_multiple_of(TIME_SLICE_TICKS)

// 5. main.rs: collapsible_if (all_user_tasks_exited 中)
//    修复：合并嵌套 if let

// 6. main.rs: bind_instead_of_map (schedule 中)
//    修复：and_then(|x| Some(y)) → map(|x| y)

// 7. main.rs: doc_lazy_continuation (重复注释行)
//    修复：删除重复的 "用户任务入口 trampoline" 注释行

// --- kernel crate (6 警告已修复) ---

// 1. lib.rs: result_unit_err (7 instances)
//    修复：添加 #![allow(clippy::result_unit_err)]（教学简化）
//    影响：arch.rs, mm/mod.rs, fs/mod.rs, loader/mod.rs

// 2. mm/mod.rs: manual_div_ceil
//    修复：(total_frames + 63) / 64 → total_frames.div_ceil(64)

// 3. task/mod.rs: module_inception
//    修复：添加 #[allow(clippy::module_inception)]（教学命名）

// 4. task/scheduler.rs: should_implement_trait
//    修复：添加 #[allow(clippy::should_implement_trait)]（调度器语义）

// 5. arch/mock.rs: new_without_default (MockTrapFrame)
//    修复：添加 #[allow(clippy::new_without_default)]

// 6. task/mod.rs: new_without_default (TaskManager)
//    修复：添加 #[allow(clippy::new_without_default)]

// ============================================================================
// 质量检查规范
// ============================================================================

/// 规范说明：
///
/// 1. **unsafe 块**: 所有 unsafe 代码必须有 `// SAFETY:` 注释
///    - 解释为什么这个操作是安全的
///    - 引用相关的文档或规范
///
/// 2. **文档注释**: 所有公开 API 必须有 `///` 文档注释
///    - 模块级注释：`//!` 解释本模块教授的硬件概念
///    - 函数文档：参数、返回值、教学概念
///    - 结构体文档：用途、字段含义
///
/// 3. **命名约定**:
///    - 函数：snake_case
///    - 类型：PascalCase
///    - 常量：SCREAMING_CASE
///    - CSR 寄存器：RISC-V 规范名称 (sstatus, stvec, etc.)
///
/// 4. **Clippy**:
///    - 运行 `cargo clippy --features mock -p suba-kernel`
///    - 运行 `cargo clippy --target riscv64gc-unknown-none-elf` (os/)
///    - 零警告
///
/// 5. **测试**:
///    - 运行 `cargo test --features mock -p suba-kernel`
///    - 运行 `cargo build --target riscv64gc-unknown-none-elf` (os/)
///    - 全部通过

// ============================================================================
// 后续改进建议
// ============================================================================

/// 当前 clippy 使用 #[allow(...)] 抑制的警告，教学版有意保留：
///
/// - `result_unit_err`: 避免过早引入错误类型设计
///   改进方向：定义 `KernelError` 枚举，包含各类错误变体
///
/// - `new_without_default`: Mock 类型不需要 Default
///   改进方向：为 Mock 类型实现 `Default` trait
///
/// - `module_inception`: task/mod.rs → pub mod task
///   改进方向：重构模块结构（但 suba 中有意为之，让学生理解子模块）
///
/// - `should_implement_trait`: scheduler.next()
///   改进方向：实现 `Iterator` trait 或更名为 `next_task()`

// ============================================================================
// SAFETY 注释统计
// ============================================================================

/// kernel/ 中所有 unsafe 块都有 SAFETY 注释：
///   arch/mock.rs: 0 unsafe blocks (纯 mock)
///   arch.rs: trait 定义, 无 unsafe 实现
///   mm/mod.rs: 每个 unsafe 块有 SAFETY 注释
///   task/mod.rs: 无 unsafe 块
///   syscall/mod.rs: 无 unsafe 块
///   fs/mod.rs: 无 unsafe 块
///   loader/mod.rs: 每个 unsafe 块有 SAFETY 注释
///
/// os/ 中所有 unsafe 块都有 SAFETY 注释：
///   main.rs: 每个 unsafe 有简短注释
///   arch/riscv64/mod.rs: 每个 CSR 操作有 SAFETY 注释
///   arch/riscv64/switch.S: 汇编级别，注释在对应 Rust 调用处
///   arch/riscv64/trap.S: 汇编级别，注释在对应 Rust 调用处
///   driver/clint.rs: MMIO 访问有 SAFETY 注释
///   driver/plic.rs: MMIO 访问有 SAFETY 注释
///   driver/uart.rs: MMIO 访问有 SAFETY 注释
