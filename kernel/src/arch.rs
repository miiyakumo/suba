//! # 架构抽象层 (arch)
//!
//! 本模块定义了内核与硬件之间的接口——HAL (Hardware Abstraction Layer)。
//!
//! ## 教学概念
//! - **CpuOps**：最底层的 CPU 操作抽象（中断开关、核心 ID、停机）
//! - **Arch**：顶层架构抽象，组合 CpuOps 并添加上下文切换、用户/内核内存复制
//! - **HwTrapFrame**：陷阱帧的统一接口，保存/恢复寄存器状态
//! - **Context**：上下文切换时保存的最小寄存器集合（ra, sp, s0-s11）
//! - **SyscallFrame**：系统调用参数的抽象访问接口
//!
//! ## 设计参考
//! HAL 采用三级 trait 层次：CpuOps → MmOps → Arch，
//! 参考 moss-kernel 的 HAL 设计文档和 comix 的实现。

pub mod mock;

// ---------------------------------------------------------------------------
// 地址类型
// ---------------------------------------------------------------------------

/// 物理地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PA(pub usize);

/// 内核虚拟地址
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct VA(pub usize);

impl PA {
    /// 创建物理地址
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }
    /// 获取地址值
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl VA {
    /// 创建虚拟地址
    pub const fn new(addr: usize) -> Self {
        Self(addr)
    }
    /// 获取地址值
    pub const fn as_usize(self) -> usize {
        self.0
    }
}

// ---------------------------------------------------------------------------
// 页表标志
// ---------------------------------------------------------------------------

/// 页表项权限标志
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PteFlags(pub usize);

impl PteFlags {
    /// 有效位
    pub const VALID: usize = 1 << 0;
    /// 可读
    pub const READ: usize = 1 << 1;
    /// 可写
    pub const WRITE: usize = 1 << 2;
    /// 可执行
    pub const EXECUTE: usize = 1 << 3;
    /// 用户态可访问
    pub const USER: usize = 1 << 4;
    /// 全局映射
    pub const GLOBAL: usize = 1 << 5;

    /// 创建新的标志
    pub const fn new(bits: usize) -> Self {
        Self(bits)
    }
    /// 获取标志位
    pub const fn bits(self) -> usize {
        self.0
    }
    /// 是否包含指定标志
    pub const fn contains(self, flag: usize) -> bool {
        (self.0 & flag) != 0
    }
}

// ---------------------------------------------------------------------------
// CpuOps
// ---------------------------------------------------------------------------

/// CPU 操作抽象 trait。
///
/// 将架构相关操作缩小到最少 6 个方法，使得同步原语等模块完全可移植。
/// 这是移植新架构时第一个需要实现的 trait。
pub trait CpuOps: 'static {
    /// 获取当前 CPU 核心 ID
    fn id() -> usize;

    /// 停止 CPU，永不返回
    fn halt() -> !;

    /// 禁用中断并返回之前的中断状态
    fn disable_interrupts() -> usize;

    /// 恢复之前保存的中断状态
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `flags` 来自 `disable_interrupts` 的返回值。
    unsafe fn restore_interrupt_state(flags: usize);

    /// 显式启用中断
    fn enable_interrupts();

    /// 当前中断是否处于启用状态
    fn interrupts_enabled() -> bool;
}

// ---------------------------------------------------------------------------
// MmOps — 内存管理操作
// ---------------------------------------------------------------------------

/// 内存管理操作 trait。
///
/// 提供地址空间相关的架构操作，如地址转换、TLB 管理等。
pub trait MmOps: 'static {
    /// 将虚拟地址翻译为物理地址
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址有效。
    unsafe fn translate_va(va: VA) -> Option<PA>;

    /// 刷新 TLB
    fn flush_tlb();

    /// 刷新指定地址的 TLB 条目
    fn flush_tlb_addr(addr: usize);

    /// 获取当前页表根物理地址
    fn current_page_table() -> PA;

    /// 切换页表
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `pt_root` 指向有效的页表。
    unsafe fn switch_page_table(pt_root: PA);
}

// ---------------------------------------------------------------------------
// Context — 上下文切换寄存器
// ---------------------------------------------------------------------------

/// 上下文切换时保存的寄存器。
///
/// 仅包含 callee-saved 寄存器（ra, sp, s0-s11），
/// 这是 Voluntary Context Switch 所需的最小集合。
///
/// ## 教学概念
/// 上下文切换不需要保存所有寄存器——caller-saved 寄存器由编译器
/// 在函数调用时自动保存到栈上，只需保存 callee-saved 寄存器。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// 返回地址
    pub ra: usize,
    /// 栈指针
    pub sp: usize,
    /// s0-s11 callee-saved 寄存器
    pub s: [usize; 12],
}

impl Context {
    /// 创建全零初始化的上下文
    pub const fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// 设置初始上下文（入口点和栈顶）
    pub fn set_init_context(&mut self, entry: usize, stack_top: usize) {
        self.ra = entry;
        self.sp = stack_top;
    }
}

// ---------------------------------------------------------------------------
// SyscallFrame — 系统调用参数抽象
// ---------------------------------------------------------------------------

/// 系统调用帧 trait。
///
/// 将系统调用参数的访问抽象化，使系统调用分发逻辑与具体架构解耦。
/// 参考 comix 的 SyscallFrame 设计。
pub trait SyscallFrame {
    /// 获取系统调用号（通常来自 a7 寄存器）
    fn syscall_id(&self) -> usize;
    /// 获取第 0 个参数（a0）
    fn arg0(&self) -> usize;
    /// 获取第 1 个参数（a1）
    fn arg1(&self) -> usize;
    /// 获取第 2 个参数（a2）
    fn arg2(&self) -> usize;
    /// 获取第 3 个参数（a3）
    fn arg3(&self) -> usize;
    /// 获取第 4 个参数（a4）
    fn arg4(&self) -> usize;
    /// 获取第 5 个参数（a5）
    fn arg5(&self) -> usize;
    /// 设置返回值（写入 a0）
    fn set_ret(&mut self, val: usize);
}

// ---------------------------------------------------------------------------
// HwTrapFrame — 陷阱帧统一接口
// ---------------------------------------------------------------------------

/// 陷阱帧的统一接口。
///
/// 保存/恢复陷阱发生时的全部寄存器状态。
/// 不同架构有不同的寄存器布局，但接口统一。
pub trait HwTrapFrame: Copy + Clone + 'static {
    /// 全零初始化
    fn zero_init() -> Self;

    /// 设置内核态陷阱帧
    fn set_kernel_trap_frame(&mut self, entry: usize, terminal: usize, kernel_sp: usize);

    /// 获取栈指针
    fn get_sp(&self) -> usize;
    /// 设置栈指针
    fn set_sp(&mut self, val: usize);

    /// 设置 a0 寄存器
    fn set_a0(&mut self, val: usize);
    /// 设置 a1 寄存器
    fn set_a1(&mut self, val: usize);
    /// 设置 a2 寄存器
    fn set_a2(&mut self, val: usize);
    /// 设置返回地址寄存器
    fn set_ra(&mut self, val: usize);

    /// 设置程序计数器（sepc）
    fn set_sepc(&mut self, pc: usize);
    /// 获取程序计数器
    fn get_sepc(&self) -> usize;
}

// ---------------------------------------------------------------------------
// Arch — 顶层架构抽象
// ---------------------------------------------------------------------------

/// 顶层架构抽象 trait。
///
/// 组合了 `CpuOps` 和 `MmOps`，并添加了上下文切换、
/// 用户/内核内存复制等架构级操作。
///
/// 内核其余部分通过 `Arch` trait 访问所有架构功能。
pub trait Arch: CpuOps + MmOps {
    /// 硬件陷阱帧类型
    type TrapFrame: HwTrapFrame + SyscallFrame;

    /// 架构名称（如 "riscv64", "mock"）
    fn name() -> &'static str;

    /// CPU 核心数量
    fn cpu_count() -> usize;

    /// 上下文切换
    ///
    /// 保存当前执行上下文到 `old`，恢复 `new` 的执行。
    ///
    /// # Safety
    ///
    /// 调用者必须确保 `new` 指向有效的上下文。
    unsafe fn context_switch(old: *mut Context, new: *const Context);

    /// 从用户空间复制数据到内核空间
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址有效且缓冲区足够大。
    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()>;

    /// 从内核空间复制数据到用户空间
    ///
    /// # Safety
    ///
    /// 调用者必须确保地址有效且缓冲区足够大。
    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()>;
}
