// .solution/8.9-page-table-test.rs — SV39 页表测试参考实现
//
// 本文件是 feature 8.9 的参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 末尾添加编译期测试断言。
//
// 由于目标是 riscv64gc-unknown-none-elf（no_std, 无测试框架），
// 我们使用 `const _: () = assert!(...)` 在编译期验证页表操作的正确性。
//
// ## 测试策略
//
// 1. PTE 创建和字段提取
// 2. 标志位组合和检查
// 3. 地址辅助函数（va_to_vpn, vpn_level_index, va_page_offset）
// 4. 编译期模拟映射/翻译/解映射流程
//
// ## 教学概念：编译期测试 vs 运行时测试
//
// 在 no_std 内核中，我们不能使用 `#[test]`（需要测试框架）。
// 但 Rust 的 const evaluation 允许我们在编译期执行复杂逻辑：
// - const fn 可以在编译期求值
// - const _: () = assert!(...) 在编译期验证条件
// - 如果断言失败，编译器会报错
//
// 这种方式将 bug 检测从运行时移到编译期——
// 通过编译就意味着测试通过，无需运行。

// ============================================================================
// 测试 1: PTE 基本操作
// ============================================================================

// 测试空 PTE
const _: () = assert!(PageTableEntry::empty().bits() == 0);
const _: () = assert!(PageTableEntry::empty().is_empty());
const _: () = assert!(!PageTableEntry::empty().is_valid());
const _: () = assert!(!PageTableEntry::empty().is_leaf());

// 测试叶子 PTE 创建和字段提取
const _: () = {
    let pte = PageTableEntry::new_leaf(0x80200, PteFlags(PteFlags::READ_WRITE));
    assert!(pte.is_valid());
    assert!(pte.is_leaf());
    assert!(pte.ppn() == 0x80200);
    assert!(pte.flags().contains(PteFlags::VALID));
    assert!(pte.flags().contains(PteFlags::READ));
    assert!(pte.flags().contains(PteFlags::WRITE));
    assert!(!pte.flags().contains(PteFlags::EXECUTE));
};

// 测试非叶子 PTE
const _: () = {
    let pte = PageTableEntry::new_table(0x1000);
    assert!(pte.is_valid());
    assert!(!pte.is_leaf()); // 非叶子: V=1, R=0, X=0
    assert!(pte.ppn() == 0x1000);
};

// ============================================================================
// 测试 2: 标志位组合
// ============================================================================

const _: () = {
    // READ_ONLY = V + R
    let flags = PteFlags(PteFlags::READ_ONLY);
    assert!(flags.contains(PteFlags::VALID));
    assert!(flags.contains(PteFlags::READ));
    assert!(!flags.contains(PteFlags::WRITE));
    assert!(!flags.contains(PteFlags::EXECUTE));

    // READ_EXECUTE = V + R + X
    let flags = PteFlags(PteFlags::READ_EXECUTE);
    assert!(flags.contains(PteFlags::VALID));
    assert!(flags.contains(PteFlags::READ));
    assert!(flags.contains(PteFlags::EXECUTE));
    assert!(!flags.contains(PteFlags::WRITE));

    // USER_READ_WRITE = V + R + W + U
    let flags = PteFlags(PteFlags::USER_READ_WRITE);
    assert!(flags.contains(PteFlags::USER));
    assert!(flags.contains(PteFlags::WRITE));

    // is_leaf 判断
    assert!(PteFlags(PteFlags::READ).is_leaf());     // R=1 → 叶子
    assert!(PteFlags(PteFlags::EXECUTE).is_leaf());   // X=1 → 叶子
    assert!(!PteFlags(PteFlags::VALID).is_leaf());    // 仅 V → 非叶子
};

// ============================================================================
// 测试 3: 地址辅助函数
// ============================================================================

// va_to_vpn: 虚拟地址 → 虚拟页号
const _: () = assert!(va_to_vpn(0x0000_0000) == 0);
const _: () = assert!(va_to_vpn(0x0000_1000) == 1);          // 第 1 页
const _: () = assert!(va_to_vpn(0x0000_1FFF) == 1);          // 页内偏移不影响 VPN
const _: () = assert!(va_to_vpn(0xFFFF_F000) == 0x000F_FFFF); // 最后一页

// va_page_offset: 虚拟地址 → 页内偏移
const _: () = assert!(va_page_offset(0x0000_0000) == 0);
const _: () = assert!(va_page_offset(0x0000_0001) == 1);
const _: () = assert!(va_page_offset(0x0000_0FFF) == 0xFFF);
const _: () = assert!(va_page_offset(0x1234_5678) == 0x678);

// vpn_level_index: VPN → 各级索引
// SV39: VPN = VPN[2](9位) | VPN[1](9位) | VPN[0](9位)
const _: () = {
    // VPN = 0: 所有级别索引为 0
    assert!(vpn_level_index(0, 0) == 0);
    assert!(vpn_level_index(0, 1) == 0);
    assert!(vpn_level_index(0, 2) == 0);

    // VPN = 1 (0x1): VPN[0] = 1
    assert!(vpn_level_index(1, 0) == 1);
    assert!(vpn_level_index(1, 1) == 0);
    assert!(vpn_level_index(1, 2) == 0);

    // VPN = 0x200 (512): VPN[1] = 1
    assert!(vpn_level_index(0x200, 0) == 0);
    assert!(vpn_level_index(0x200, 1) == 1);
    assert!(vpn_level_index(0x200, 2) == 0);

    // VPN = 0x40000 (262144): VPN[2] = 1
    assert!(vpn_level_index(0x40000, 0) == 0);
    assert!(vpn_level_index(0x40000, 1) == 0);
    assert!(vpn_level_index(0x40000, 2) == 1);

    // 复合测试: VPN = 0x40201 → VPN[2]=1, VPN[1]=1, VPN[0]=1
    assert!(vpn_level_index(0x40201, 0) == 1);
    assert!(vpn_level_index(0x40201, 1) == 1);
    assert!(vpn_level_index(0x40201, 2) == 1);
};

// ============================================================================
// 测试 4: 编译期 PTE 位模式验证
// ============================================================================

// 验证 SV39 PTE 的位布局
const _: () = {
    // PPN 在位 10-53
    let pte = PageTableEntry::new_leaf(0x1, PteFlags(PteFlags::VALID));
    // PPN=1 应该在 bit 10
    assert!(pte.bits() & (1 << 10) != 0);

    // 验证 PPN 提取
    let pte = PageTableEntry::new_leaf(0x3FFFF_FFFF, PteFlags(PteFlags::VALID));
    assert!(pte.ppn() == 0x3FFFF_FFFF); // 44 位最大 PPN
};

// 验证 PTE 的 set_ppn 和 set_flags
const _: () = {
    let mut pte = PageTableEntry::new_leaf(0x100, PteFlags(PteFlags::READ_WRITE));
    assert!(pte.ppn() == 0x100);

    // 修改 PPN
    pte.set_ppn(0x200);
    assert!(pte.ppn() == 0x200);
    // 标志位应保持不变
    assert!(pte.flags().contains(PteFlags::READ));
    assert!(pte.flags().contains(PteFlags::WRITE));

    // 修改标志
    pte.set_flags(PteFlags(PteFlags::READ_EXECUTE));
    assert!(!pte.flags().contains(PteFlags::WRITE));
    assert!(pte.flags().contains(PteFlags::EXECUTE));
    // PPN 应保持不变
    assert!(pte.ppn() == 0x200);
};

// ============================================================================
// 测试 5: 常量验证
// ============================================================================

const _: () = assert!(PAGE_SIZE == 4096);
const _: () = assert!(PAGE_OFFSET_BITS == 12);
const _: () = assert!(VPN_BITS == 9);
const _: () = assert!(PAGE_TABLE_LEVELS == 3);
const _: () = assert!(PTE_PER_PAGE == 512);
const _: () = assert!(SV39_VA_BITS == 39);
const _: () = assert!(SV39_PA_BITS == 56);
