// .solution/9.1-uart-registers.rs — UART MMIO 寄存器定义参考实现
//
// 本文件是 feature 9.1 的参考实现。
// 学生应在 os/src/driver/uart.rs 中定义 NS16550A UART 的 MMIO 寄存器结构。
//
// ## 实现要点
//
// 1. 寄存器偏移量常量 (reg 模块)
// 2. LSR/IER 标志位定义
// 3. Uart 结构体：封装基地址，提供 volatile 读写方法
// 4. 编译期断言验证常量正确性
//
// ## 教学概念：MMIO (Memory-Mapped I/O)
//
// MMIO 将硬件寄存器映射到物理地址空间：
// - 读地址 = 从硬件寄存器读取值
// - 写地址 = 向硬件寄存器写入值
//
// 关键：必须使用 volatile 操作，否则编译器可能优化掉必要的读写。
// 例如，如果编译器认为"读取结果未使用"，可能直接删掉读操作——
// 但 MMIO 读取本身就有副作用（清除中断标志等）。
//
// ## NS16550A 寄存器布局
//
// 偏移  名称  读/写  说明
// +0    RBR   读     接收缓冲区
// +0    THR   写     发送保持
// +1    IER   读/写  中断使能
// +2    IIR   读     中断标识
// +2    FCR   写     FIFO 控制
// +3    LCR   读/写  线路控制
// +4    MCR   读/写  调制解调器控制
// +5    LSR   读     线路状态
// +6    MSR   读     调制解调器状态
// +7    SCR   读/写  临时寄存器

use core::ptr::{read_volatile, write_volatile};

pub const UART0_BASE: usize = 0x1000_0000;

pub mod reg {
    pub const RBR: usize = 0;
    pub const THR: usize = 0;
    pub const IER: usize = 1;
    pub const IIR: usize = 2;
    pub const FCR: usize = 2;
    pub const LCR: usize = 3;
    pub const MCR: usize = 4;
    pub const LSR: usize = 5;
    pub const MSR: usize = 6;
    pub const SCR: usize = 7;
}

pub struct LsrFlags;
impl LsrFlags {
    pub const DR: u8 = 1 << 0;
    pub const THRE: u8 = 1 << 5;
    pub const TEMT: u8 = 1 << 6;
}

pub struct IerFlags;
impl IerFlags {
    pub const ERBFI: u8 = 1 << 0;
    pub const ETBEI: u8 = 1 << 1;
    pub const ELSI: u8 = 1 << 2;
    pub const EDSSI: u8 = 1 << 3;
}

pub struct Uart {
    base: usize,
}

impl Uart {
    pub const fn new(base: usize) -> Self {
        Self { base }
    }

    unsafe fn read_reg(&self, offset: usize) -> u8 {
        unsafe { read_volatile((self.base + offset) as *const u8) }
    }

    unsafe fn write_reg(&self, offset: usize, val: u8) {
        unsafe { write_volatile((self.base + offset) as *mut u8, val) }
    }

    pub fn read_rbr(&self) -> u8 {
        unsafe { self.read_reg(reg::RBR) }
    }

    pub fn write_thr(&self, c: u8) {
        unsafe { self.write_reg(reg::THR, c) }
    }

    pub fn read_lsr(&self) -> u8 {
        unsafe { self.read_reg(reg::LSR) }
    }

    pub fn write_ier(&self, val: u8) {
        unsafe { self.write_reg(reg::IER, val) }
    }

    pub fn read_iir(&self) -> u8 {
        unsafe { self.read_reg(reg::IIR) }
    }

    pub fn write_fcr(&self, val: u8) {
        unsafe { self.write_reg(reg::FCR, val) }
    }

    pub fn write_lcr(&self, val: u8) {
        unsafe { self.write_reg(reg::LCR, val) }
    }

    pub fn write_mcr(&self, val: u8) {
        unsafe { self.write_reg(reg::MCR, val) }
    }

    pub fn is_thr_empty(&self) -> bool {
        self.read_lsr() & LsrFlags::THRE != 0
    }

    pub fn is_data_ready(&self) -> bool {
        self.read_lsr() & LsrFlags::DR != 0
    }
}
