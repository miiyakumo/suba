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
