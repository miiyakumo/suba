// .solution/10.2-program-headers.rs — 程序头解析参考实现
//
// 本文件是 feature 10.2 的参考实现。
// 学生应在 kernel/src/loader/mod.rs 中实现程序头遍历。
//
// ## 教学概念：程序头表（Program Header Table）
//
// 程序头表是 ELF 文件中"段"（Segment）的目录。
// 加载器遍历这个表，找到所有 `PT_LOAD` 类型的段，
// 然后将它们从文件复制到内存中正确的虚拟地址。
//
// 程序头表紧跟在 ELF 头之后（或在 e_phoff 指定的位置）：
//
//   ```text
//   ELF Header (64 bytes)
//   Program Header 0  ← e_phoff
//   Program Header 1
//   ...
//   Program Header N-1
//   ```
//
// 每个程序头 56 字节（ELF64），描述一个段的加载方式。

// === 程序头结构体 ===

/// ELF 64 位程序头（56 字节）
///
/// ## 教学概念：Segment vs Section
///
/// ELF 有两种"分组"概念：
/// - **Section**（节）：链接器使用，描述代码/数据的逻辑分区（.text, .data, .bss）
/// - **Segment**（段）：加载器使用，描述如何映射到内存
///
/// 一个 Segment 可以包含多个 Section。
/// 加载器只关心 Segment（程序头），不关心 Section（节头）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProgramHeader64 {
    pub p_type: u32,    // 段类型（PT_LOAD, PT_DYNAMIC, ...）
    pub p_flags: u32,   // 权限标志（R, W, X）
    pub p_offset: u64,  // 段在文件中的偏移
    pub p_vaddr: u64,   // 段在内存中的虚拟地址
    pub p_paddr: u64,   // 段在内存中的物理地址（通常不用）
    pub p_filesz: u64,  // 段在文件中的大小
    pub p_memsz: u64,   // 段在内存中的大小（>= p_filesz 的部分清零 = BSS）
    pub p_align: u64,   // 对齐要求
}

// === 段类型常量 ===

pub mod segment_type {
    pub const PT_LOAD: u32 = 1;
    pub const PT_DYNAMIC: u32 = 2;
    pub const PT_INTERP: u32 = 3;
}

// === 段权限标志 ===

pub mod segment_flags {
    pub const PF_X: u32 = 1;  // 可执行
    pub const PF_W: u32 = 2;  // 可写
    pub const PF_R: u32 = 4;  // 可读
}

impl ProgramHeader64 {
    /// 从原始字节解析一个程序头条目
    pub fn from_bytes(data: &[u8]) -> Result<Self, crate::loader::ElfError> {
        if data.len() < core::mem::size_of::<ProgramHeader64>() {
            return Err(crate::loader::ElfError::TooShort);
        }
        // SAFETY: 长度已检查 >= 56，ProgramHeader64 是 #[repr(C)] POD 类型
        let phdr = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ProgramHeader64)
        };
        Ok(phdr)
    }

    /// 是否为 PT_LOAD 段
    pub fn is_load(&self) -> bool {
        self.p_type == segment_type::PT_LOAD
    }

    pub fn offset(&self) -> usize { self.p_offset as usize }
    pub fn vaddr(&self) -> usize { self.p_vaddr as usize }
    pub fn file_size(&self) -> usize { self.p_filesz as usize }
    pub fn mem_size(&self) -> usize { self.p_memsz as usize }
    pub fn is_readable(&self) -> bool { self.p_flags & segment_flags::PF_R != 0 }
    pub fn is_writable(&self) -> bool { self.p_flags & segment_flags::PF_W != 0 }
    pub fn is_executable(&self) -> bool { self.p_flags & segment_flags::PF_X != 0 }
}

// === 便捷函数 ===

/// 从 ELF 数据中解析所有程序头
///
/// 遍历程序头表，收集所有条目到 Vec。
pub fn parse_program_headers(
    data: &[u8],
    phoff: u64,
    phentsize: u16,
    phnum: u16,
) -> Result<alloc::vec::Vec<ProgramHeader64>, crate::loader::ElfError> {
    let phoff = phoff as usize;
    let entsize = phentsize as usize;
    let count = phnum as usize;

    if entsize < core::mem::size_of::<ProgramHeader64>() {
        return Err(crate::loader::ElfError::PhdrOutOfBounds);
    }

    let table_end = phoff
        .checked_add(entsize.checked_mul(count).ok_or(crate::loader::ElfError::PhdrOutOfBounds)?)
        .ok_or(crate::loader::ElfError::PhdrOutOfBounds)?;
    if table_end > data.len() {
        return Err(crate::loader::ElfError::PhdrOutOfBounds);
    }

    let mut phdrs = alloc::vec::Vec::with_capacity(count);
    for i in 0..count {
        let offset = phoff + i * entsize;
        let phdr = ProgramHeader64::from_bytes(&data[offset..offset + entsize])?;
        phdrs.push(phdr);
    }
    Ok(phdrs)
}
