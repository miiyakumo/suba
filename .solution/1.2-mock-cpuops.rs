/// Mock CPU 实现。
///
/// 所有操作都是 no-op 或返回固定值，仅用于测试。
pub struct MockCpuOps;

impl CpuOps for MockCpuOps {
    fn id() -> usize {
        0
    }

    fn halt() -> ! {
        loop {
            core::hint::spin_loop();
        }
    }

    fn disable_interrupts() -> usize {
        // Mock: 中断总是禁用的
        0
    }

    unsafe fn restore_interrupt_state(_flags: usize) {
        // Mock: 无操作
    }

    fn enable_interrupts() {
        // Mock: 无操作
    }

    fn interrupts_enabled() -> bool {
        false
    }
}
