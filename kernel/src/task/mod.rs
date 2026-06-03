//! # 任务管理 (task)
//!
//! 本模块实现了多任务调度的核心逻辑。
//!
//! ## 教学概念
//! - **Task Control Block (TCB)**：每个任务的所有信息集中在一个结构体中
//! - **TaskState**：任务状态机（Ready → Running → Waiting → Exited）
//! - **上下文切换**：保存/恢复 callee-saved 寄存器，实现任务切换
//! - **Round-Robin 调度**：最简单的调度算法——轮流执行
//!
//! ## 教学故事
//! "有了多个任务之后，如何在它们之间切换？"
//! 答案：保存当前任务的寄存器，恢复下一个任务的寄存器——就这么简单。

pub mod task;
pub mod scheduler;

pub use task::{Task, TaskState};
pub use scheduler::RoundRobinScheduler;
