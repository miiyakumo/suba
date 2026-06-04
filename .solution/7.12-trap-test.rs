// .solution/7.12-trap-test.rs — trap 汇编偏移量验证参考实现
//
// 本文件是 feature 7.12 的完整参考实现。
// 学生应在 os/src/arch/riscv64/mod.rs 中添加编译期偏移量断言。

// ============================================================================
// 编译期偏移量验证
// ============================================================================

// ## 教学概念：编译期偏移量验证
//
// trap.S 通过硬编码偏移量（如 `sd ra, 8(a0)`）访问 TrapFrame 字段。
// 如果 Rust 结构体的字段顺序或大小与汇编代码不匹配，会导致数据错位——
// 这种 bug 极难调试。
//
// 我们使用 `core::mem::offset_of!` 宏在编译期验证每个字段的偏移量。
// 如果任何断言失败，编译会报错，而不是在运行时出现神秘的 crash。
//
// 这是 OS 开发中的最佳实践：将"信任边界"从运行时移到编译期。

// 1. 验证总大小：34 个字段 × 8 字节 = 272 字节
const _: () = assert!(core::mem::size_of::<TrapFrame>() == 272);

// 2. 验证每个字段的偏移量与 trap.S 中的注释一致
//    trap.S 使用 `sd reg, OFFSET(a0)` 保存寄存器
//    trap.S 使用 `ld reg, OFFSET(a0)` 恢复寄存器
const _: () = assert!(core::mem::offset_of!(TrapFrame, sepc) == 0);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x1_ra) == 8);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x2_sp) == 16);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x3_gp) == 24);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x4_tp) == 32);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x5_t0) == 40);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x6_t1) == 48);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x7_t2) == 56);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x8_s0) == 64);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x9_s1) == 72);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x10_a0) == 80);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x11_a1) == 88);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x12_a2) == 96);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x13_a3) == 104);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x14_a4) == 112);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x15_a5) == 120);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x16_a6) == 128);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x17_a7) == 136);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x18_s2) == 144);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x19_s3) == 152);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x20_s4) == 160);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x21_s5) == 168);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x22_s6) == 176);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x23_s7) == 184);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x24_s8) == 192);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x25_s9) == 200);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x26_s10) == 208);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x27_s11) == 216);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x28_t3) == 224);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x29_t4) == 232);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x30_t5) == 240);
const _: () = assert!(core::mem::offset_of!(TrapFrame, x31_t6) == 248);
const _: () = assert!(core::mem::offset_of!(TrapFrame, sstatus) == 256);
const _: () = assert!(core::mem::offset_of!(TrapFrame, kernel_sp) == 264);
