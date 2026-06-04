//! # 参考实现：ELF 段加载到内存
//!
//! 本文件展示如何将 ELF 的 PT_LOAD 段复制到用户地址空间。
//!
//! ## 核心思路
//!
//! 对每个 PT_LOAD 段：
//! 1. 转换 ELF 权限标志 (PF_R/W/X) → PTE 标志 (READ/WRITE/EXECUTE)
//! 2. 逐页分配物理帧（alloc_zeroed_frame 自动清零 → BSS 天然为零）
//! 3. 从 ELF 数据复制文件内容到物理帧（内核身份映射，直接写 PA）
//! 4. 映射物理帧到用户虚拟地址空间（自动添加 U 标志）

use suba_kernel::loader::ProgramHeader64;

/// 将 PT_LOAD 段加载到用户地址空间
pub fn load_segments(
    elf_data: &[u8],
    phdrs: &[ProgramHeader64],
    user_space: &crate::arch::riscv64::page::UserAddrSpace,
) -> Result<(), ()> {
    use crate::arch::riscv64::page::{PteFlags, PAGE_SIZE, alloc_zeroed_frame};
    use suba_kernel::loader::segment_flags;

    for phdr in phdrs {
        if !phdr.is_load() {
            continue;
        }

        let vaddr = phdr.vaddr();
        let file_size = phdr.file_size();
        let mem_size = phdr.mem_size();
        let offset = phdr.offset();

        if file_size > mem_size {
            return Err(());
        }

        // 转换 ELF 标志 → PTE 标志
        let mut pte_bits = PteFlags::VALID;
        if phdr.p_flags & segment_flags::PF_R != 0 { pte_bits |= PteFlags::READ; }
        if phdr.p_flags & segment_flags::PF_W != 0 { pte_bits |= PteFlags::WRITE; }
        if phdr.p_flags & segment_flags::PF_X != 0 { pte_bits |= PteFlags::EXECUTE; }
        let flags = PteFlags(pte_bits);

        let num_pages = mem_size.div_ceil(PAGE_SIZE);

        for i in 0..num_pages {
            let page_va = vaddr + i * PAGE_SIZE;
            let page_file_offset = i * PAGE_SIZE;

            // 分配物理帧并清零
            let ppn = alloc_zeroed_frame().ok_or(())?;
            let pa = ppn * PAGE_SIZE;

            // 复制文件数据
            if page_file_offset < file_size {
                let src_start = offset + page_file_offset;
                let copy_len = core::cmp::min(PAGE_SIZE, file_size - page_file_offset);
                let src_end = src_start + copy_len;
                if src_end > elf_data.len() { return Err(()); }

                // SAFETY: pa 是刚分配的物理帧，身份映射下 VA=PA
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        elf_data[src_start..src_end].as_ptr(),
                        pa as *mut u8,
                        copy_len,
                    );
                }
            }

            // 映射到用户地址空间
            // SAFETY: pa 指向刚分配的有效物理帧
            unsafe {
                user_space.map_user_page(page_va, pa, flags).map_err(|_| ())?;
            }
        }
    }

    Ok(())
}
