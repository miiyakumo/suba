// Solution: 10.8 - 实现 RISC-V Arch trait 完整实现
//
// 本文件展示如何为 Riscv64 实现 Arch trait。
// 完整代码位于 os/src/arch/riscv64/mod.rs。

// ============================================================================
// Riscv64Arch — 整合所有 RISC-V 后端组件
// ============================================================================

pub struct Riscv64Arch;

// --- CpuOps 实现 ---

impl suba_kernel::arch::CpuOps for Riscv64Arch {
    fn id() -> usize { 0 }

    fn halt() -> ! {
        loop {
            unsafe { core::arch::asm!("wfi", options(nomem, nostack)) }
        }
    }

    fn enable_interrupts() {
        unsafe { core::arch::asm!("csrsi sstatus, 0x2", options(nomem, nostack)) }
    }

    fn disable_interrupts() -> usize {
        let old: usize;
        unsafe {
            core::arch::asm!("csrrci {}, sstatus, 0x2", out(reg) old, options(nomem, nostack));
        }
        old & 0x2
    }

    fn interrupts_enabled() -> bool {
        let sstatus: usize;
        unsafe { core::arch::asm!("csrr {}, sstatus", out(reg) sstatus, options(nomem, nostack)) }
        sstatus & 0x2 != 0
    }

    unsafe fn restore_interrupt_state(flags: usize) {
        if flags != 0 {
            unsafe { core::arch::asm!("csrsi sstatus, 0x2", options(nomem, nostack)) }
        } else {
            unsafe { core::arch::asm!("csrci sstatus, 0x2", options(nomem, nostack)) }
        }
    }
}

// --- MmOps 实现 ---

impl suba_kernel::arch::MmOps for Riscv64Arch {
    unsafe fn translate_va(va: suba_kernel::arch::VA) -> Option<suba_kernel::arch::PA> {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF;
        match page::translate_va(va.as_usize(), root_ppn, page::phys_read_u64) {
            Ok(pa) => Some(suba_kernel::arch::PA::new(pa)),
            Err(_) => None,
        }
    }

    fn flush_tlb() { flush_tlb_all(); }
    fn flush_tlb_addr(addr: usize) { flush_tlb_addr(addr); }

    fn current_page_table() -> suba_kernel::arch::PA {
        let satp = read_satp();
        let root_ppn = satp & 0x0FFF_FFFF_FFFF;
        suba_kernel::arch::PA::new(root_ppn << 12)
    }

    unsafe fn switch_page_table(pt_root: suba_kernel::arch::PA) {
        let root_ppn = pt_root.as_usize() >> 12;
        unsafe { switch_page_table(root_ppn); }
    }
}

// --- Arch 实现 ---

impl suba_kernel::arch::Arch for Riscv64Arch {
    type TrapFrame = TrapFrame;

    fn name() -> &'static str { "riscv64" }
    fn cpu_count() -> usize { 1 }

    /// 上下文切换：调用 switch.S 保存/恢复 callee-saved 寄存器
    unsafe fn context_switch(old: *mut suba_kernel::arch::Context, new: *const suba_kernel::arch::Context) {
        unsafe { switch(old, new); }
    }

    /// 从用户空间复制到内核空间
    /// 翻译用户 VA → PA，通过内核身份映射复制
    unsafe fn copy_from_user(src: usize, dst: *mut u8, len: usize) -> Result<(), ()> {
        // 逐页翻译并复制
        // translate_user_va() 使用当前 satp 页表翻译
    }

    /// 从内核空间复制到用户空间
    unsafe fn copy_to_user(src: *const u8, dst: usize, len: usize) -> Result<(), ()> {
        // 与 copy_from_user 对称
    }
}
