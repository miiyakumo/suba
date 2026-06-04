// .solution/10.3-load-segments.rs — 段加载到内存参考实现
//
// 本文件是 feature 10.3 的参考实现。
// 学生应在 kernel/src/loader/mod.rs 中实现段加载。
//
// ## 实现要点
//
// 1. copy_segment() — 将单个 PT_LOAD 段从 ELF 数据复制到目标内存
// 2. load_elf_segments() — 高层接口：解析 ELF → 遍历 PT_LOAD → 复制段
// 3. BSS 处理：p_memsz > p_filesz 时，多余部分清零
//
// ## 教学概念：段加载过程
//
// 加载器将 ELF 文件中的 PT_LOAD 段复制到内存中正确的位置。
// 过程如下：
//
//   ELF 文件                     内存 (p_vaddr)
//   ┌──────────────┐            ┌──────────────┐
//   │ p_offset     │  ──copy──→ │ p_vaddr      │  p_filesz 字节
//   │ (p_filesz)   │            │              │
//   └──────────────┘            ├──────────────┤
//                               │  BSS 区域    │  (p_memsz - p_filesz) 字节清零
//                               │  (零填充)    │
//                               └──────────────┘
//
// 当 p_memsz > p_filesz 时，多出的部分是 BSS 段（未初始化全局变量），
// 加载器必须将其清零。这是 C 语言规范的要求。

// === copy_segment 参考实现 ===
//
// pub unsafe fn copy_segment(
//     data: &[u8],
//     phdr: &ProgramHeader64,
//     dst: *mut u8,
// ) -> Result<(), ElfError> {
//     let offset = phdr.offset();
//     let filesz = phdr.file_size();
//     let memsz = phdr.mem_size();
//
//     let end = offset.checked_add(filesz).ok_or(ElfError::PhdrOutOfBounds)?;
//     if end > data.len() {
//         return Err(ElfError::PhdrOutOfBounds);
//     }
//
//     if filesz > 0 {
//         // SAFETY: 调用者确保 dst 足够大，data 范围已验证
//         unsafe {
//             core::ptr::copy_nonoverlapping(
//                 data[offset..offset + filesz].as_ptr(),
//                 dst,
//                 filesz,
//             );
//         }
//     }
//
//     if memsz > filesz {
//         let bss_start = dst.add(filesz);
//         let bss_size = memsz - filesz;
//         // SAFETY: 调用者确保 dst 指向至少 memsz 字节的可写内存
//         unsafe {
//             core::ptr::write_bytes(bss_start, 0, bss_size);
//         }
//     }
//
//     Ok(())
// }
