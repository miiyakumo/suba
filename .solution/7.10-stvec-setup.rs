// .solution/7.10-stvec-setup.rs — stvec 陷阱向量设置参考实现
//
// 本文件展示如何在 rust_main 中设置 stvec CSR 为 trap_entry 地址。

// 在 os/src/arch/riscv64/mod.rs 中添加 init_trap 函数：

/// 初始化陷阱处理：设置 stvec CSR 为 trap_entry 地址
///
/// ## 教学概念：stvec CSR
///
/// stvec (Supervisor Trap Vector) 寄存器告诉 CPU：
/// "当发生陷阱时，跳转到这个地址"。
///
/// stvec 的格式：
/// - bits 63:2: 基地址（必须 4 字节对齐）
/// - bits 1:0: 陷阱模式
///   - 0 (Direct): 所有陷阱跳转到基地址
///   - 1 (Vectored): 中断跳转到 base+4*cause，异常跳转到 base
///
/// 我们使用 Direct 模式——所有陷阱统一进入 trap_entry，
/// 由软件根据 scause 分发。这比 Vectored 模式更灵活。
pub fn init_trap() {
    // SAFETY: 写入 stvec 是初始化阶段的安全操作
    unsafe {
        asm!(
            "csrw stvec, {addr}",
            addr = in(reg) trap_entry as usize,
            options(nomem, nostack)
        );
    }
}

// 在 os/src/main.rs 的 rust_main 中调用：
//
//   // ---- Step 3: 设置陷阱向量 ----
//   arch::riscv64::init_trap();
//   uart_puts("[suba] trap vector set\n");
