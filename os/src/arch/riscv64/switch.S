# 上下文切换 — RISC-V 64 位
#
#   fn switch(old_ctx: *mut Context, new_ctx: *const Context);
#
# 将当前 CPU 上下文（callee-saved 寄存器）保存到 old_ctx，
# 然后从 new_ctx 恢复寄存器并跳转到 new_ctx.ra 继续执行。
#
# ## 教学概念
#
# RISC-V 调用约定中，寄存器分为两类：
# - **caller-saved** (t0-t6, a0-a7): 调用者负责保存，编译器在函数调用前自动压栈
# - **callee-saved** (s0-s11): 被调用者负责保存，跨调用需要手动保存/恢复
#
# 上下文切换只需保存 callee-saved 寄存器（14 个 = 112 字节），
# 因为 caller-saved 寄存器已经由编译器在栈上保存了。
#
# ra (返回地址) 和 sp (栈指针) 也必须保存：
# - ra: 切换后从哪里恢复执行
# - sp: 每个任务有自己的内核栈
#
# ## Context 结构体布局 (#[repr(C)])
#
# 偏移量   字段
# 0        ra
# 8        sp
# 16       s[0]  (s0/fp)
# 24       s[1]  (s1)
# 32       s[2]
# ...
# 104      s[11]

    .section .text
    .globl switch
    .align 2

switch:
    # ===== 保存当前上下文到 old_ctx (a0) =====
    # TODO(student): 用 sd 指令保存 ra, sp, s0-s11 到 a0 指向的 Context
    # Context 布局: ra=offset 0, sp=offset 8, s0=offset 16, s1=offset 24, ...
    # 提示: sd rs, offset(base) — 将寄存器 rs 存储到 base+offset 地址

    sd ra,  0(a0)       # 保存返回地址
    sd sp,  8(a0)       # 保存栈指针
    sd s0,  16(a0)      # 保存 s0 (frame pointer)
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
    # TODO(student): 用 ld 指令从 a1 指向的 Context 恢复 ra, sp, s0-s11
    # 提示: ld rd, offset(base) — 从 base+offset 地址加载到寄存器 rd

    ld ra,  0(a1)       # 恢复返回地址
    ld sp,  8(a1)       # 恢复栈指针
    ld s0,  16(a1)
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

    # ===== 跳转到新上下文 =====
    # TODO(student): ret 指令跳转到恢复的 ra
    # ra 已被 new_ctx 的值覆盖，ret 跳转到 new_ctx.ra
    ret
