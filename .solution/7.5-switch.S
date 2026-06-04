# 上下文切换
#
#   fn switch(old_ctx: *mut Context, new_ctx: *const Context);
#
# RISC-V calling convention:
#   a0 = old_ctx 指针（第一个参数）
#   a1 = new_ctx 指针（第二个参数）
#
# Context 结构体布局 (#[repr(C)]):
#   offset  0: ra   (返回地址)
#   offset  8: sp   (栈指针)
#   offset 16: s0   (callee-saved)
#   offset 24: s1
#   offset 32: s2
#   offset 40: s3
#   offset 48: s4
#   offset 56: s5
#   offset 64: s6
#   offset 72: s7
#   offset 80: s8
#   offset 88: s9
#   offset 96: s10
#   offset 104: s11
#
# ## 教学概念
# 上下文切换只需保存 callee-saved 寄存器（ra, sp, s0-s11）。
# caller-saved 寄存器（t0-t6, a0-a7）由编译器在函数调用时自动压栈，
# 因此不需要在上下文切换时显式保存。
#
# 共 14 个寄存器 x 8 字节 = 112 字节 = sizeof(Context)

.section .text
.globl switch
switch:
    # ===== 保存当前上下文到 old_ctx (a0) =====
    sd ra,  0(a0)      # 保存返回地址
    sd sp,  8(a0)      # 保存栈指针
    sd s0,  16(a0)     # 保存 s0-s11
    sd s1,  24(a0)
    sd s2,  32(a0)
    sd s3,  40(a0)
    sd s4,  48(a0)
    sd s5,  56(a0)
    sd s6,  64(a0)
    sd s7,  72(a0)
    sd s8,  80(a0)
    sd s9,  88(a0)
    sd s10, 96(a0)
    sd s11, 104(a0)

    # ===== 从 new_ctx (a1) 恢复上下文 =====
    ld ra,  0(a1)      # 恢复返回地址
    ld sp,  8(a1)      # 恢复栈指针
    ld s0,  16(a1)     # 恢复 s0-s11
    ld s1,  24(a1)
    ld s2,  32(a1)
    ld s3,  40(a1)
    ld s4,  48(a1)
    ld s5,  56(a1)
    ld s6,  64(a1)
    ld s7,  72(a1)
    ld s8,  80(a1)
    ld s9,  88(a1)
    ld s10, 96(a1)
    ld s11, 104(a1)

    # 跳转到 new_ctx 的 ra（即恢复的任务的上次执行点）
    ret
