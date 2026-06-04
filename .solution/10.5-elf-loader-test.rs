// Solution: 10.5 - ELF 加载器测试
//
// 本文件展示添加的测试用例。完整代码位于 kernel/src/loader/mod.rs 的 tests 模块。
//
// 新增测试用例（8 个）：
// 1. parse_program_headers_mixed_types — 混合类型程序头（PT_LOAD + PT_DYNAMIC）
// 2. copy_segment_bss_only — 纯 BSS 段（filesz=0, memsz>0）
// 3. copy_segment_zero_memsz — 空段（filesz=0, memsz=0）
// 4. parse_program_headers_zero_count — 空程序头表（phnum=0）
// 5. parse_program_headers_larger_entsize — 条目大于最小值（entsize=64 > 56）
// 6. program_header_flags_all_combinations — 所有 R/W/X 标志组合
// 7. program_header_non_load_types — 非 PT_LOAD 类型验证
// 8. load_elf_segments_with_two_segments — 完整的两段加载流程

// --- 测试: 纯 BSS 段 ---
// 验证 filesz=0, memsz=16 的 BSS 段被正确清零
#[test]
fn copy_segment_bss_only() {
    let mut buf = [0u8; 56];
    buf[0..4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
    buf[4..8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_W).to_le_bytes());
    buf[32..40].copy_from_slice(&0u64.to_le_bytes());  // p_filesz = 0
    buf[40..48].copy_from_slice(&16u64.to_le_bytes());  // p_memsz = 16

    let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
    let mut dst = [0xFFu8; 16]; // 预填充非零值

    unsafe { copy_segment(&[], &phdr, dst.as_mut_ptr()).unwrap(); }

    // BSS 段应被清零
    for i in 0..16 {
        assert_eq!(dst[i], 0);
    }
}

// --- 测试: 空段 ---
// 验证 filesz=0, memsz=0 的空段成功但不修改目标
#[test]
fn copy_segment_zero_memsz() {
    let mut buf = [0u8; 56];
    buf[0..4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
    buf[32..40].copy_from_slice(&0u64.to_le_bytes());
    buf[40..48].copy_from_slice(&0u64.to_le_bytes());

    let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
    let mut dst = [0xABu8; 4];

    unsafe { copy_segment(&[], &phdr, dst.as_mut_ptr()).unwrap(); }
    assert_eq!(dst, [0xABu8; 4]); // 保持不变
}

// --- 测试: 完整两段加载 ---
// 构建包含 .text + .data 的 ELF，分别加载并验证内容
#[test]
fn load_elf_segments_with_two_segments() {
    // ... 完整实现见 kernel/src/loader/mod.rs
}
