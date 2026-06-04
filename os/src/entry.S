    .section .text.entry
    .globl _start
_start:
    # ========== 1. 设置栈指针 ==========
    # CPU 从 OpenSBI 跳转到此处时，sp 未初始化
    # boot_stack_top 是链接脚本中定义的栈顶地址
    la sp, boot_stack_top

    # ========== 2. 清零 BSS 段 ==========
    # BSS 段存放未初始化的全局/静态变量，RISC-V 规范要求它们为零
    # sbss/ebss 是链接脚本中 BSS 段的起止地址
    la t0, sbss              # t0 = BSS 起始地址
    la t1, ebss              # t1 = BSS 结束地址
.Lclear_bss:
    bgeu t0, t1, .Lbss_done  # 如果 t0 >= t1，清零完成
    sd zero, 0(t0)           # 将 8 字节零写入 [t0]
    addi t0, t0, 8           # t0 += 8
    j .Lclear_bss            # 继续循环
.Lbss_done:

    # ========== 3. 跳转到 Rust 入口 ==========
    call rust_main

    # rust_main 不应返回；若返回则停机
.Lhalt:
    wfi
    j .Lhalt

    # ========== 栈空间 ==========
    .section .bss.stack
    .globl boot_stack_lower_bound
boot_stack_lower_bound:
    .space 4096 * 16         # 16 页 = 64KB 内核栈
    .globl boot_stack_top
boot_stack_top: