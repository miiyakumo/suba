//! # suba-kernel — 平台无关的内核核心
//!
//! 本 crate 包含教学内核的所有平台无关逻辑。
//! 通过 HAL trait（`CpuOps`、`Arch`）与具体硬件解耦，
//! 支持两种后端：
//! - **Mock 后端**：在宿主机 `cargo test --features mock` 运行，零硬件依赖
//! - **RISC-V 后端**：由 `os` crate 提供，在 QEMU/真实硬件上运行
//!
//! ## 教学概念
//! - `#![no_std]`：裸机环境没有标准库，需要自建一切
//! - 模块化设计：每个模块教授一个 OS 概念
//! - trait 抽象：用 Rust trait 实现架构无关的核心逻辑

#![no_std]
#![warn(missing_docs)]
// 教学简化：Result<(), ()> 避免定义复杂错误类型
#![allow(clippy::result_unit_err)]

extern crate alloc;

pub mod arch;
pub mod driver;
pub mod mm;
pub mod task;
pub mod syscall;
pub mod fs;
pub mod loader;
