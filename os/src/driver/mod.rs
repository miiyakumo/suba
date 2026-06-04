//! # 硬件驱动 (driver)
//!
//! 本模块包含 RISC-V 硬件平台的设备驱动。
//!
//! ## 教学概念：MMIO (Memory-Mapped I/O)
//!
//! 在 RISC-V 中，硬件设备的寄存器被映射到物理地址空间。
//! 通过读写这些"内存地址"来控制硬件——这就是 MMIO。
//!
//! ```text
//! QEMU virt 机器 MMIO 布局:
//!   0x1000_0000  UART0 (NS16550A)
//!   0x0200_0000  CLINT (Core Local Interruptor)
//!   0x0C00_0000  PLIC (Platform-Level Interrupt Controller)
//! ```
//!
//! 与普通内存不同，MMIO 地址的读写有副作用（会改变硬件状态），
//! 因此必须使用 `volatile` 访问，防止编译器优化掉"看似无用"的读写。

pub mod clint;
pub mod plic;
pub mod uart;

/// UART Console 类型别名
///
/// 实现了 kernel 的 `Console` trait，用于 RISC-V 硬件上的串口 I/O。
#[allow(dead_code)]
pub type UartConsole = uart::UartConsole;
