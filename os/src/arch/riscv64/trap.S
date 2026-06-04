# trap.S — RISC-V 陷阱入口和恢复
#
# 当 CPU 发生陷阱（中断/异常/系统调用）时，硬件跳转到 stvec 指向的地址。
# 本文件实现两个核心汇编函数：
#   - trap_entry: 保存全部寄存器到 TrapFrame，切换到内核栈，调用 trap_handler
#   - trap_return:  从 TrapFrame 恢复全部寄存器，执行 sret 返回
#
# ## 教学概念：RISC-V 陷阱处理流程
#
# 1. CPU 发生陷阱 → 硬件自动设置：
#    - sepc ← PC（当前指令地址）
#    - scause ← 陷阱原因
#    - stval ← 附加信息（如故障地址）
#    - sstatus.SPP ← 当前特权级
#    - sstatus.SPIE ← sstatus.SIE
#    - sstatus.SIE ← 0（关闭中断）
#    - PC ← stvec（跳转到陷阱入口）
#
# 2. 软件（trap_entry）必须：
#    - 保存所有通用寄存器（硬件不保存！）
#    - 保存 sepc、sstatus（后续恢复用）
#    - 切换到内核栈（如果从用户态进入）
#    - 调用 Rust 的 trap_handler
#
# 3. 处理完毕后（trap_return）：
#    - 恢复所有寄存器
#    - 执行 sret：PC ← sepc，SIE ← SPIE，特权级 ← SPP
#
# ## TrapFrame 布局（#[repr(C)], 34 × 8 = 272 字节）
#
# offset   字段
# 0        sepc
# 8        x1 (ra)
# 16       x2 (sp)
# 24       x3 (gp)
# 32       x4 (tp)
# 40       x5 (t0)
# 48       x6 (t1)
# 56       x7 (t2)
# 64       x8 (s0/fp)
# 72       x9 (s1)
# 80       x10 (a0)
# 88       x11 (a1)
# 96       x12 (a2)
# 104      x13 (a3)
# 112      x14 (a4)
# 120      x15 (a5)
# 128      x16 (a6)
# 136      x17 (a7)
# 144      x18 (s2)
# 152      x19 (s3)
# 160      x20 (s4)
# 168      x21 (s5)
# 176      x22 (s6)
# 184      x23 (s7)
# 192      x24 (s8)
# 200      x25 (s9)
# 208      x26 (s10)
# 216      x27 (s11)
# 224      x28 (t3)
# 232      x29 (t4)
# 240      x30 (t5)
# 248      x31 (t6)
# 256      sstatus
# 264      kernel_sp

    .section .text
    .globl trap_entry
    .globl trap_return
    .align 2

# ============================================================================
# trap_entry — 陷阱入口
# ============================================================================
# 硬件通过 stvec 跳转到此处。
# 此时 sscratch 指向当前任务的 TrapFrame。
#
# 步骤：
# 1. 交换 sscratch 和 a0（a0 ← TrapFrame 指针）
# 2. 保存全部 31 个通用寄存器到 TrapFrame
# 3. 保存 sepc、sstatus CSR
# 4. 如果从用户态进入（SPP=0），切换到内核栈
# 5. 调用 Rust 的 trap_handler(trap_frame_ptr)
trap_entry:
    # ---- Step 1: 获取 TrapFrame 指针 ----
    # TODO(student): 用 csrrw 交换 a0 和 sscratch
    # 交换后：a0 = TrapFrame 指针，sscratch = 原 a0 值
    # 提示: csrrw rd, csr, rs — 原子交换 rd 和 csr 的值
    csrrw a0, sscratch, a0

    # ---- Step 2: 保存全部通用寄存器 ----
    # TODO(student): 用 sd 指令保存所有寄存器到 a0 指向的 TrapFrame
    # 注意：a0 此时是 TrapFrame 指针，原 a0 值在 sscratch 中
    # TrapFrame 布局: sepc=offset 0, ra=offset 8, sp=offset 16, ...
    # 提示: sd rs, offset(base) — 将寄存器 rs 存储到 base+offset 地址

    # 保存 sepc（陷阱返回地址）
    csrr t0, sepc
    sd t0, 0(a0)

    # 保存通用寄存器（除了 a0，后面单独处理）
    sd ra,  8(a0)
    sd sp,  16(a0)
    sd gp,  24(a0)
    sd tp,  32(a0)
    sd t0,  40(a0)
    sd t1,  48(a0)
    sd t2,  56(a0)
    sd s0,  64(a0)
    sd s1,  72(a0)
    # a0 在 Step 3 保存（先从 sscratch 恢复原值）
    sd a1,  88(a0)
    sd a2,  96(a0)
    sd a3,  104(a0)
    sd a4,  112(a0)
    sd a5,  120(a0)
    sd a6,  128(a0)
    sd a7,  136(a0)
    sd s2,  144(a0)
    sd s3,  152(a0)
    sd s4,  160(a0)
    sd s5,  168(a0)
    sd s6,  176(a0)
    sd s7,  184(a0)
    sd s8,  192(a0)
    sd s9,  200(a0)
    sd s10, 208(a0)
    sd s11, 216(a0)
    sd t3,  224(a0)
    sd t4,  232(a0)
    sd t5,  240(a0)
    sd t6,  248(a0)

    # ---- Step 3: 保存原 a0 和 sstatus ----
    # TODO(student): 从 sscratch 读取原 a0 值并保存到 TrapFrame.x10_a0
    # 提示: csrr rd, csr — 读取 CSR 到寄存器
    csrr t0, sscratch
    sd t0, 80(a0)

    # TODO(student): 保存 sstatus CSR 到 TrapFrame.sstatus (offset 256)
    csrr t0, sstatus
    sd t0, 256(a0)

    # ---- Step 4: 切换到内核栈（仅用户态进入时） ----
    # TODO(student): 检查 sstatus.SPP 位 (bit 8)，决定是否切换栈
    # SPP=1: 陷阱来自 S-mode（内核态），不切换栈
    # SPP=0: 陷阱来自 U-mode（用户态），需要切换到内核栈
    # 提示:
    #   1. csrr t0, sstatus — 读取 sstatus
    #   2. li t1, 0x100     — SPP 位掩码 (1 << 8)
    #   3. and t0, t0, t1   — 提取 SPP 位
    #   4. bnez t0, .Lkernel_trap — SPP=1 跳过栈切换
    #   5. ld sp, 264(a0)   — 从 TrapFrame.kernel_sp 加载内核栈
    csrr t0, sstatus
    li t1, 0x100
    and t0, t0, t1
    bnez t0, .Lkernel_trap

    # 来自用户态：切换到内核栈
    ld sp, 264(a0)

.Lkernel_trap:
    # 保存当前 sp 到 kernel_sp（无论是否切换了栈）
    # 这确保 trap_handler 返回后栈是正确的
    sd sp, 264(a0)

    # ---- Step 5: 调用 trap_handler ----
    # TODO(student): 跳转到 Rust 的 trap_handler 函数
    # a0 已经是 TrapFrame 指针，直接作为参数传递
    # 提示: 使用 call 宏调用 trap_handler（等价于 auipc+jal）
    # 注意: a0 (TrapFrame 指针) 在调用后可能被破坏，
    #       但由于 trap_entry → trap_handler → trap_return 是连续执行，
    #       trap_handler 内部会调用 trap_return 完成恢复
    call trap_handler

    # trap_handler 不应返回；若返回则停机
    wfi
    j .-4

# ============================================================================
# trap_return — 陷阱恢复
# ============================================================================
# 从 TrapFrame 恢复全部寄存器并执行 sret。
# 调用约定：a0 = TrapFrame 指针
#
# 步骤：
# 1. 恢复 sepc、sstatus CSR
# 2. 将 TrapFrame 指针写入 sscratch（下次陷阱用）
# 3. 恢复全部通用寄存器
# 4. sret 返回
trap_return:
    # ---- Step 1: 恢复 CSR ----
    # TODO(student): 从 TrapFrame 恢复 sepc 和 sstatus
    # 提示: ld t0, offset(a0) 然后 csrw csr, t0
    ld t0, 0(a0)
    csrw sepc, t0

    ld t0, 256(a0)
    csrw sstatus, t0

    # ---- Step 2: 设置 sscratch ----
    # TODO(student): 将 TrapFrame 指针写回 sscratch（供下次陷阱使用）
    # 提示: csrw sscratch, a0
    csrw sscratch, a0

    # ---- Step 3: 恢复通用寄存器 ----
    # TODO(student): 用 ld 指令从 TrapFrame 恢复所有通用寄存器
    # 注意：a0 必须最后恢复（因为它是 TrapFrame 指针）
    # 提示: ld rd, offset(a0) — 从 a0+offset 加载到寄存器
    ld ra,  8(a0)
    ld sp,  16(a0)
    ld gp,  24(a0)
    ld tp,  32(a0)
    ld t0,  40(a0)
    ld t1,  48(a0)
    ld t2,  56(a0)
    ld s0,  64(a0)
    ld s1,  72(a0)
    # a0 最后恢复（因为它是 TrapFrame 指针）
    ld a1,  88(a0)
    ld a2,  96(a0)
    ld a3,  104(a0)
    ld a4,  112(a0)
    ld a5,  120(a0)
    ld a6,  128(a0)
    ld a7,  136(a0)
    ld s2,  144(a0)
    ld s3,  152(a0)
    ld s4,  160(a0)
    ld s5,  168(a0)
    ld s6,  176(a0)
    ld s7,  184(a0)
    ld s8,  192(a0)
    ld s9,  200(a0)
    ld s10, 208(a0)
    ld s11, 216(a0)
    ld t3,  224(a0)
    ld t4,  232(a0)
    ld t5,  240(a0)
    ld t6,  248(a0)

    # 最后恢复 a0（从 TrapFrame.x10_a0 字段）
    ld a0,  80(a0)

    # ---- Step 4: 陷阱返回 ----
    # TODO(student): 执行 sret 指令返回
    # sret 硬件自动完成：PC ← sepc, SIE ← SPIE, 特权级 ← SPP
    sret
