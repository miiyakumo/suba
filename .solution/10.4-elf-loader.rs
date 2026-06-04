// Solution: 10.4 - 完整的 ElfLoader 实现
//
// 本文件展示如何在 RISC-V 后端实现 ElfLoader trait。
// 完整代码位于 os/src/arch/riscv64/mod.rs。

// ============================================================================
// Riscv64ElfLoader — 实现 ElfLoader trait
// ============================================================================

/// RISC-V 64 位 ELF 加载器
///
/// 实现 kernel crate 的 `ElfLoader` trait，整合：
/// - ELF 头解析（parse_elf_header）
/// - 程序头解析（parse_program_headers）
/// - 用户地址空间创建（UserAddrSpace::new）
/// - 段加载（load_segments）
/// - 用户栈映射（map_user_stack）
pub struct Riscv64ElfLoader;

impl suba_kernel::loader::ElfLoader for Riscv64ElfLoader {
    fn load_elf(data: &[u8]) -> Result<(usize, usize), ()> {
        use suba_kernel::loader::{parse_elf_header, parse_program_headers};

        // Step 1: 解析并验证 ELF 头
        let (entry, phoff, phentsize, phnum) = parse_elf_header(data).map_err(|_| ())?;

        // Step 2: 解析所有程序头
        let phdrs = parse_program_headers(data, phoff, phentsize, phnum).map_err(|_| ())?;

        // Step 3: 创建用户地址空间（含内核映射复制）
        let user_space = page::UserAddrSpace::new().map_err(|_| ())?;

        // Step 4: 加载 PT_LOAD 段到物理帧并映射到用户页表
        load_segments(data, &phdrs, &user_space)?;

        // Step 5: 映射用户栈（8MB，从 USER_STACK_TOP 向下增长）
        let user_stack_top = user_space.map_user_stack().map_err(|_| ())?;

        Ok((entry as usize, user_stack_top))
    }
}

// ============================================================================
// load_elf_to_space — 返回 UserAddrSpace 的扩展版本
// ============================================================================

/// 从 ELF 数据加载用户程序，返回完整的地址空间
///
/// 与 `Riscv64ElfLoader::load_elf` 类似，但额外返回 `UserAddrSpace`。
/// 调用者需要持有地址空间来切换页表（`user_space.activate()`）。
pub fn load_elf_to_space(
    data: &[u8],
) -> Result<(page::UserAddrSpace, usize, usize), ()> {
    use suba_kernel::loader::{parse_elf_header, parse_program_headers};

    let (entry, phoff, phentsize, phnum) = parse_elf_header(data).map_err(|_| ())?;
    let phdrs = parse_program_headers(data, phoff, phentsize, phnum).map_err(|_| ())?;
    let user_space = page::UserAddrSpace::new().map_err(|_| ())?;
    load_segments(data, &phdrs, &user_space)?;
    let user_stack_top = user_space.map_user_stack().map_err(|_| ())?;

    Ok((user_space, entry as usize, user_stack_top))
}

// ============================================================================
// 使用示例（在 rust_main 中）
// ============================================================================

// fn load_and_run_user_program(elf_data: &[u8]) {
//     // 加载 ELF 到用户地址空间
//     let (user_space, entry, stack_top) =
//         arch::riscv64::load_elf_to_space(elf_data).expect("ELF load failed");
//
//     // 切换到用户页表
//     unsafe { user_space.activate(); }
//
//     // 切换到用户态
//     arch::riscv64::enter_user_mode(entry, stack_top, kernel_stack_top);
// }
