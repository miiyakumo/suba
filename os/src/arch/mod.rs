//! 架构抽象层
//!
//! 为不同硬件架构提供统一的内核接口。
//! 当前仅支持 RISC-V 64 位。

#[cfg(target_arch = "riscv64")]
pub mod riscv64;
