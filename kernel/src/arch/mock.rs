//! # Mock 架构实现
//!
//! 在宿主机（非 RISC-V）上编译时，提供 Mock 实现使得架构无关代码可以
//! 编译和测试。这些 mock 实现仅在 `#[cfg(feature = "mock")]` 时激活。
//!
//! ## 教学概念
//! Mock 测试是"前半学期"的核心——学生在宿主机上 `cargo test` 完成所有实验，
//! 零硬件依赖。切换到真实硬件只需改编译参数，一行业务代码都不用改。

use super::{Arch, CpuOps, MmOps, HwTrapFrame, SyscallFrame, Context, PA, VA};

// ============================================================================
// MockCpuOps — Mock CPU 实现
// ============================================================================

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

// ============================================================================
// MockMmOps — Mock 内存管理实现
// ============================================================================

/// Mock 内存管理实现。
pub struct MockMmOps;

impl MmOps for MockMmOps {
    unsafe fn translate_va(_va: VA) -> Option<PA> {
        // Mock: 直接映射，虚拟地址 == 物理地址
        Some(PA::new(_va.as_usize()))
    }

    fn flush_tlb() {
        // Mock: 无操作
    }

    fn flush_tlb_addr(_addr: usize) {
        // Mock: 无操作
    }

    fn current_page_table() -> PA {
        PA::new(0)
    }

    unsafe fn switch_page_table(_pt_root: PA) {
        // Mock: 无操作
    }
}

// ============================================================================
// MockTrapFrame — Mock 陷阱帧
// ============================================================================

/// Mock 陷阱帧。
///
/// 保存系统调用参数和返回值，用于测试系统调用分发逻辑。
#[derive(Debug, Clone, Copy)]
pub struct MockTrapFrame {
    /// 系统调用号
    pub syscall_no: usize,
    /// 参数 0-5
    pub args: [usize; 6],
    /// 返回值
    pub ret: usize,
    /// 程序计数器
    pub pc: usize,
    /// 栈指针
    pub sp: usize,
}

impl MockTrapFrame {
    /// 创建新的 Mock 陷阱帧
    pub fn new() -> Self {
        Self {
            syscall_no: 0,
            args: [0; 6],
            ret: 0,
            pc: 0,
            sp: 0,
        }
    }
}

impl HwTrapFrame for MockTrapFrame {
    fn zero_init() -> Self {
        Self::new()
    }

    fn set_kernel_trap_frame(&mut self, entry: usize, _terminal: usize, kernel_sp: usize) {
        self.pc = entry;
        self.sp = kernel_sp;
    }

    fn get_sp(&self) -> usize {
        self.sp
    }

    fn set_sp(&mut self, val: usize) {
        self.sp = val;
    }

    fn set_a0(&mut self, val: usize) {
        self.args[0] = val;
    }

    fn set_a1(&mut self, val: usize) {
        self.args[1] = val;
    }

    fn set_a2(&mut self, val: usize) {
        self.args[2] = val;
    }

    fn set_ra(&mut self, _val: usize) {
        // Mock: 无操作
    }

    fn set_sepc(&mut self, pc: usize) {
        self.pc = pc;
    }

    fn get_sepc(&self) -> usize {
        self.pc
    }
}

impl SyscallFrame for MockTrapFrame {
    fn syscall_id(&self) -> usize {
        self.syscall_no
    }

    fn arg0(&self) -> usize {
        self.args[0]
    }
    fn arg1(&self) -> usize {
        self.args[1]
    }
    fn arg2(&self) -> usize {
        self.args[2]
    }
    fn arg3(&self) -> usize {
        self.args[3]
    }
    fn arg4(&self) -> usize {
        self.args[4]
    }
    fn arg5(&self) -> usize {
        self.args[5]
    }

    fn set_ret(&mut self, val: usize) {
        self.ret = val;
    }
}

// ============================================================================
// MockArch — 组合 Mock 实现
// ============================================================================

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
        unsafe { core::ptr::copy_nonoverlapping(src as *const u8, dst, len) };
        Ok(())
    }

    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
        if len != 0 && (src.is_null() || dst == 0) {
            return Err(());
        }
        unsafe { core::ptr::copy_nonoverlapping(src, dst as *mut u8, len) };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mock_cpu_ops() {
        assert_eq!(MockArch::id(), 0);
        assert!(!MockArch::interrupts_enabled());
    }

    #[test]
    fn mock_trap_frame_syscall() {
        let mut tf = MockTrapFrame::new();
        tf.syscall_no = 64; // sys_write
        tf.args[0] = 1; // fd
        tf.args[1] = 0x8000_0000; // buf
        tf.args[2] = 13; // len
        assert_eq!(tf.syscall_id(), 64);
        assert_eq!(tf.arg0(), 1);
        assert_eq!(tf.arg1(), 0x8000_0000);
        assert_eq!(tf.arg2(), 13);
        tf.set_ret(13);
        assert_eq!(tf.ret, 13);
    }

    #[test]
    fn context_init() {
        let mut ctx = Context::zero_init();
        assert_eq!(ctx.ra, 0);
        assert_eq!(ctx.sp, 0);
        ctx.set_init_context(0x8020_0000, 0x8040_0000);
        assert_eq!(ctx.ra, 0x8020_0000);
        assert_eq!(ctx.sp, 0x8040_0000);
    }
}
