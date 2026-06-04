// Solution: 10.6 - 创建用户栈
//
// 本文件展示如何在 rust_main 中创建用户地址空间并映射用户栈。
// 完整代码位于 os/src/main.rs。

// 在 rust_main() 中，页表测试之后添加：

// ---- Step 5.5: 用户栈创建测试 ----
// 创建用户地址空间并映射用户栈，验证栈可用
uart_puts("[suba] creating user stack...\n");
match arch::riscv64::page::UserAddrSpace::new() {
    Ok(user_space) => {
        match user_space.map_user_stack() {
            Ok(stack_top) => {
                uart_puts("[suba] user stack mapped: top=");
                arch::riscv64::page::print_hex(stack_top);
                uart_puts(", size=8MB\n");
            }
            Err(_) => {
                uart_puts("[suba] WARN: user stack mapping failed\n");
            }
        }
    }
    Err(_) => {
        uart_puts("[suba] WARN: user addr space creation failed\n");
    }
}

// ============================================================================
// 教学概念：用户栈布局
// ============================================================================
//
// 用户栈在用户地址空间的高地址区域，从 USER_STACK_TOP 向下增长：
//
// ```text
// USER_TOP (0x0000_0040_0000_0000)
//   ┌──────────────────────┐
//   │ trampoline 页        │  ← trap_entry 代码（用户/内核共享）
//   ├──────────────────────┤
//   │ USER_STACK_TOP       │  ← 栈顶（高地址）
//   │   ↓ 用户栈向下增长    │
//   │   ...                │
//   │   [8MB 栈空间]       │
//   │   ...                │
//   │ 栈底                 │  ← USER_STACK_TOP - USER_STACK_SIZE
//   ├──────────────────────┤
//   │                      │
//   │ 用户代码/数据         │  ← ELF PT_LOAD 段
//   │                      │
//   └──────────────────────┘
// ```
//
// 栈大小 8MB = 2048 个 4KB 页，足够大多数用户程序使用。
// 栈从高地址向低地址增长（RISC-V 约定），
// sp 寄存器初始值 = USER_STACK_TOP。
