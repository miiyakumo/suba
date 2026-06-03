/// Mock 架构实现，组合 MockCpuOps 和 MockMmOps。
pub struct MockArch;

impl CpuOps for MockArch {
    fn id() -> usize { MockCpuOps::id() }
    fn halt() -> ! { MockCpuOps::halt() }
    fn disable_interrupts() -> usize { MockCpuOps::disable_interrupts() }
    unsafe fn restore_interrupt_state(flags: usize) {
        // SAFETY: 调用者确保 flags 来自 disable_interrupts
        unsafe { MockCpuOps::restore_interrupt_state(flags) }
    }
    fn enable_interrupts() { MockCpuOps::enable_interrupts() }
    fn interrupts_enabled() -> bool { MockCpuOps::interrupts_enabled() }
}

impl MmOps for MockArch {
    unsafe fn translate_va(va: VA) -> Option<PA> {
        // SAFETY: Mock 环境下地址恒等映射
        unsafe { MockMmOps::translate_va(va) }
    }
    fn flush_tlb() { MockMmOps::flush_tlb() }
    fn flush_tlb_addr(addr: usize) { MockMmOps::flush_tlb_addr(addr) }
    fn current_page_table() -> PA { MockMmOps::current_page_table() }
    unsafe fn switch_page_table(pt_root: PA) {
        // SAFETY: Mock 环境下无实际操作
        unsafe { MockMmOps::switch_page_table(pt_root) }
    }
}

impl Arch for MockArch {
    type TrapFrame = MockTrapFrame;

    fn name() -> &'static str {
        "mock"
    }

    fn cpu_count() -> usize {
        1
    }

    unsafe fn context_switch(_old: *mut Context, _new: *const Context) {
        // Mock: 无操作
    }

    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
        if len != 0 && (src == 0 || dst.is_null()) {
            return Err(());
        }
        // SAFETY: 调用者确保 src 和 dst 有效且 len 不越界
        unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
        Ok(())
    }

    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
        if len != 0 && (src.is_null() || dst == 0) {
            return Err(());
        }
        // SAFETY: 调用者确保 src 和 dst 有效且 len 不越界
        unsafe { core::ptr::copy_nonoverlapping(src, dst as *mut u8, len) };
        Ok(())
    }
}
