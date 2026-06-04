# trap.S — RISC-V 陷阱入口和恢复
#
# 当 CPU 发生陷阱（中断/异常/系统调用）时，硬件跳转到 stvec 指向的地址。
# 本文件实现三个核心汇编函数：
#   - trap_entry:  保存全部寄存器到 TrapFrame，切换到内核栈，调用 trap_handler
#   - trap_return: 从 TrapFrame 恢复全部寄存器，执行 sret 返回
#   - forkret:     用户任务首次调度时的入口，设置 sscratch 后调用 trap_return
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
    .globl forkret
    .align 2

# ============================================================================
# trap_entry — 陷阱入口
# ============================================================================
trap_entry:
    # Step 1: 获取 TrapFrame 指针
    # sscratch 在进入用户态前被设置为 TrapFrame 地址
    # csrrw 原子交换 a0 和 sscratch：a0 ← sscratch (TrapFrame 指针)
    csrrw a0, sscratch, a0

    # Step 2: 保存全部通用寄存器
    csrr t0, sepc
    sd t0, 0(a0)

    sd ra,  8(a0)
    sd sp,  16(a0)
    sd gp,  24(a0)
    sd tp,  32(a0)
    sd t0,  40(a0)
    sd t1,  48(a0)
    sd t2,  56(a0)
    sd s0,  64(a0)
    sd s1,  72(a0)
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

    # Step 3: 保存原 a0 和 sstatus
    csrr t0, sscratch
    sd t0, 80(a0)

    csrr t0, sstatus
    sd t0, 256(a0)

    # Step 4: 切换到内核栈（仅用户态进入时）
    # sstatus.SPP (bit 8): 0 = 用户态, 1 = 内核态
    csrr t0, sstatus
    li t1, 0x100
    and t0, t0, t1
    bnez t0, .Lkernel_trap

    # 从用户态进入：切换到内核栈
    ld sp, 264(a0)

.Lkernel_trap:
    # 保存当前内核栈指针到 TrapFrame
    sd sp, 264(a0)

    # Step 5: 调用 trap_handler
    # trap_handler 处理陷阱（中断、异常、系统调用）。
    # 如果时钟中断触发了任务切换，trap_handler 会设置 NEXT_TRAP_FRAME。
    call trap_handler

    # Step 6: 检查 NEXT_TRAP_FRAME（抢占式调度）
    #
    # ## 教学概念：抢占式上下文切换
    #
    # 当时钟中断来自用户态时，调度器可能选择切换到另一个任务。
    # 此时 NEXT_TRAP_FRAME 被设置为下一个任务的 TrapFrame 指针。
    # trap_entry 从新的 TrapFrame 恢复寄存器并 sret 到新任务。
    #
    # 如果 NEXT_TRAP_FRAME 为 NULL，表示不需要切换（如来自内核态的中断），
    # 直接返回到中断前的执行点。
    la t0, NEXT_TRAP_FRAME
    ld a0, 0(t0)
    beqz a0, .Lreturn_to_kernel

    # NEXT_TRAP_FRAME 非空：从新任务的 TrapFrame 恢复
    # 清空 NEXT_TRAP_FRAME（一次性使用）
    sd zero, 0(t0)

    # a0 = 新任务的 TrapFrame 指针
    # fall through to trap_return

# ============================================================================
# trap_return — 陷阱恢复（供 enter_user_mode 和 forkret 调用）
# ============================================================================
# 参数：a0 = TrapFrame 指针
# 行为：恢复全部寄存器，执行 sret
trap_return:
    # 恢复 CSR
    ld t0, 0(a0)
    csrw sepc, t0
    ld t0, 256(a0)
    csrw sstatus, t0

    # 设置 sscratch（供下次从用户态陷入时使用）
    csrw sscratch, a0

    # 恢复通用寄存器（a0 最后恢复）
    ld ra,  8(a0)
    ld sp,  16(a0)
    ld gp,  24(a0)
    ld tp,  32(a0)
    ld t0,  40(a0)
    ld t1,  48(a0)
    ld t2,  56(a0)
    ld s0,  64(a0)
    ld s1,  72(a0)
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
    ld a0,  80(a0)

    # sret: PC ← sepc, SIE ← SPIE, 特权级 ← SPP
    sret

# ============================================================================
# .Lreturn_to_kernel — 返回到内核任务
# ============================================================================
# 当 trap_handler 返回 0 时，表示切换到了内核任务（如 idle）。
# 此时不需要恢复 TrapFrame（内核任务通过 context_switch 恢复上下文），
# 直接返回到调用 trap_entry 的位置（即 context_switch 后的 schedule() 函数）。
.Lreturn_to_kernel:
    ret

# ============================================================================
# forkret — 用户任务首次调度入口
# ============================================================================
# 当用户任务首次被 context_switch 调度时，context.ra 指向此处。
# forkret 的职责：
#   1. 从 Task 结构体读取 trap_frame_ptr
#   2. 将其写入 sscratch CSR
#   3. 调用 trap_return 恢复 TrapFrame 并 sret 到用户态
#
# 参数：a0 = 当前任务的 trap_frame_ptr（usize，由 schedule() 在调用前设置）
#
# ## 教学概念：为什么需要 forkret？
#
# 新创建的用户任务从未执行过，它的"上下文"是人为构造的：
# - context.ra = forkret（首次调度时跳转到这里）
# - context.sp = 内核栈顶
#
# 当 context_switch 切换到这个任务时，CPU 从 forkret 开始执行。
# forkret 设置 sscratch 并调用 trap_return，完成到用户态的切换。
# 这是任务"诞生"的时刻——从内核态切换到用户态，开始执行用户程序。
forkret:
    # a0 已经是 trap_frame_ptr（由 schedule() 设置）
    # 写入 sscratch（供 trap_return 和后续 trap_entry 使用）
    csrw sscratch, a0

    # 调用 trap_return 恢复 TrapFrame 并 sret 到用户态
    j trap_return
