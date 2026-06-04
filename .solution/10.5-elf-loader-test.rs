// Solution: 10.5 - ELF 加载器单元测试
//
// 本文件展示了 kernel/src/loader/mod.rs 中测试用例的完整参考实现。
// 完整测试代码位于 kernel/src/loader/mod.rs 的 `mod tests` 块中。
//
// 测试覆盖范围（29 个测试）：
//
// 1. 结构体大小验证
//    - elf_header_size: ElfHeader64 = 64 字节
//    - program_header_size: ProgramHeader64 = 56 字节
//
// 2. ELF 魔数验证
//    - elf_magic_check: 有效/无效魔数
//
// 3. ELF 头 from_bytes
//    - from_bytes_valid: 正常解析
//    - from_bytes_too_short: 数据不足
//
// 4. ELF 头 validate（7 个测试）
//    - validate_valid_header: 完整有效头
//    - validate_bad_magic: 错误魔数
//    - validate_not_elf64: 32 位 ELF
//    - validate_not_little_endian: 大端序
//    - validate_not_riscv: 非 RISC-V 架构
//    - validate_not_executable: 非可执行文件 (ET_REL)
//    - validate_pie_accepted: PIE (ET_DYN) 应通过
//
// 5. ELF 头字段提取
//    - extract_entry_point: 入口地址
//    - extract_phdr_info: phoff/phnum/phentsize
//
// 6. parse_elf_header（7 个测试）
//    - parse_elf_header_valid/too_short/bad_magic/not_elf64/
//      not_little_endian/not_riscv/not_executable/pie
//
// 7. 程序头解析（5 个测试）
//    - parse_program_headers_valid: 2 个段的解析
//    - parse_program_headers_out_of_bounds: phoff 越界
//    - parse_program_headers_bad_entsize: entsize 过小
//    - parse_program_headers_zero_count: phnum=0
//    - parse_program_headers_larger_entsize: entsize > 56
//
// 8. ProgramHeader64 from_bytes（2 个测试）
//    - program_header_from_bytes: 正常解析 + 标志位验证
//    - program_header_from_bytes_too_short: 数据不足
//
// 9. copy_segment（4 个测试）
//    - copy_segment_basic: 正常复制 + BSS 清零
//    - copy_segment_out_of_bounds: filesz 超出 ELF 数据
//    - copy_segment_bss_only: 纯 BSS 段 (filesz=0, memsz>0)
//    - copy_segment_zero_memsz: 空段 (filesz=0, memsz=0)
//
// 10. 标志位和类型组合（2 个测试）
//     - program_header_flags_all_combinations: 所有 R/W/X 组合
//     - program_header_non_load_types: 非 PT_LOAD 类型
//
// 11. 混合类型和完整加载（2 个测试）
//     - parse_program_headers_mixed_types: PT_LOAD + PT_DYNAMIC 混合
//     - load_elf_segments_with_two_segments: 完整的两段加载流程

// ============================================================================
// 辅助函数示例
// ============================================================================

/// 构建有效的 ELF64 RISC-V 可执行头字节
fn make_valid_elf_bytes() -> [u8; 64] {
    let mut data = [0u8; 64];
    data[0..4].copy_from_slice(b"\x7fELF");  // 魔数
    data[4] = 2;      // ELFCLASS64
    data[5] = 1;      // ELFDATA2LSB
    data[6] = 1;      // EV_CURRENT
    data[16..18].copy_from_slice(&2u16.to_le_bytes());     // ET_EXEC
    data[18..20].copy_from_slice(&243u16.to_le_bytes());   // EM_RISCV
    data[20..24].copy_from_slice(&1u32.to_le_bytes());     // e_version
    data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes()); // e_entry
    data[32..40].copy_from_slice(&64u64.to_le_bytes());    // e_phoff
    data[52..54].copy_from_slice(&64u16.to_le_bytes());    // e_ehsize
    data[54..56].copy_from_slice(&56u16.to_le_bytes());    // e_phentsize
    data[56..58].copy_from_slice(&2u16.to_le_bytes());     // e_phnum
    data
}

// ============================================================================
// 关键测试用例示例
// ============================================================================

// --- 测试: 纯 BSS 段 ---
// 验证 filesz=0, memsz=16 的 BSS 段被正确清零
#[test]
fn copy_segment_bss_only() {
    let mut buf = [0u8; 56];
    buf[0..4].copy_from_slice(&1u32.to_le_bytes());   // PT_LOAD
    buf[4..8].copy_from_slice(&6u32.to_le_bytes());   // PF_R | PF_W
    buf[32..40].copy_from_slice(&0u64.to_le_bytes());  // p_filesz = 0
    buf[40..48].copy_from_slice(&16u64.to_le_bytes()); // p_memsz = 16

    let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
    let mut dst = [0xFFu8; 16]; // 预填充非零值

    // SAFETY: dst 足够大
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
    buf[0..4].copy_from_slice(&1u32.to_le_bytes());   // PT_LOAD
    buf[32..40].copy_from_slice(&0u64.to_le_bytes());  // p_filesz = 0
    buf[40..48].copy_from_slice(&0u64.to_le_bytes());  // p_memsz = 0

    let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
    let mut dst = [0xABu8; 4];

    // SAFETY: dst 存在
    unsafe { copy_segment(&[], &phdr, dst.as_mut_ptr()).unwrap(); }
    assert_eq!(dst, [0xABu8; 4]); // 保持不变
}

// --- 测试: 完整两段加载 ---
// 构建包含 .text + .data 的 ELF，分别加载并验证内容
// 包括 BSS 清零验证（.data 段 memsz > filesz）
// 完整实现见 kernel/src/loader/mod.rs:1147-1223
