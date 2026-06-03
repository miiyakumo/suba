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
