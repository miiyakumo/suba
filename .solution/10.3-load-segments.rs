// .solution/10.3-load-segments.rs — 段加载参考实现
//
// 本文件是 feature 10.3 的参考实现。
// 学生应在 kernel/src/loader/mod.rs 中实现段加载。
//
// ## 教学概念：加载 PT_LOAD 段到内存
//
// 加载器的核心工作是将 ELF 中的 PT_LOAD 段复制到内存中正确的位置。
//
// 过程：
// 1. 解析 ELF 头 → 获取入口点和程序头表位置
// 2. 遍历程序头表 → 找到所有 PT_LOAD 段
// 3. 对每个 PT_LOAD 段：
//    a. 从 ELF 文件复制 p_filesz 字节到 p_vaddr
//    b. 将 (p_memsz - p_filesz) 字节清零（BSS 段）
//
// BSS 段（未初始化全局变量）：
// - 在 ELF 文件中不占空间（p_filesz = 0 或 < p_memsz）
// - 在内存中需要占空间（p_memsz）
// - 必须初始化为零（C 语言规范：未初始化全局变量默认为 0）
//
// ## 教学概念：为什么需要 copy_nonoverlapping？
//
// `copy_nonoverlapping` 类似 C 的 `memcpy`，但有额外保证：
// - 源和目标不能重叠（否则用 `copy`）
// - 必须在 unsafe 块中调用（因为操作原始指针）
// - 调用者必须确保内存足够大

// === copy_segment 函数 ===

/// 将 PT_LOAD 段从 ELF 数据复制到目标内存地址
///
/// # Safety
///
/// `dst` 必须指向至少 `p_memsz` 字节的可写内存。
/// 调用者负责确保目标地址已经通过页表映射到正确的物理帧。
pub unsafe fn copy_segment(
    data: &[u8],
    phdr: &ProgramHeader64,
    dst: *mut u8,
) -> Result<(), ElfError> {
    let offset = phdr.offset();
    let filesz = phdr.file_size();
    let memsz = phdr.mem_size();

    // 检查段数据是否在 ELF 文件范围内
    let end = offset.checked_add(filesz).ok_or(ElfError::PhdrOutOfBounds)?;
    if end > data.len() {
        return Err(ElfError::PhdrOutOfBounds);
    }

    // Step 1: 从 ELF 文件复制 p_filesz 字节到目标地址
    if filesz > 0 {
        // SAFETY: 调用者确保 dst 指向至少 memsz 字节的可写内存，
        // 且 src (data[offset..offset+filesz]) 是有效的读取范围。
        unsafe {
            core::ptr::copy_nonoverlapping(
                data[offset..offset + filesz].as_ptr(),
                dst,
                filesz,
            );
        }
    }

    // Step 2: 将 BSS 区域清零（p_memsz - p_filesz 字节）
    if memsz > filesz {
        let bss_size = memsz - filesz;
        // SAFETY: 调用者确保 dst 指向至少 memsz 字节的可写内存
        unsafe {
            let bss_start = dst.add(filesz);
            core::ptr::write_bytes(bss_start, 0, bss_size);
        }
    }

    Ok(())
}

// === load_elf_segments 函数 ===

/// 从 ELF 数据加载所有 PT_LOAD 段到目标内存
///
/// # Safety
///
/// `base_addr` 必须指向已经映射好的可写内存区域。
pub unsafe fn load_elf_segments(
    data: &[u8],
    base_addr: *mut u8,
) -> Result<usize, ElfError> {
    let header = ElfHeader64::from_bytes(data)?;
    header.validate()?;

    let entry = header.entry_point();
    let phdrs = parse_program_headers(
        data,
        header.e_phoff,
        header.e_phentsize,
        header.e_phnum,
    )?;

    for phdr in &phdrs {
        if !phdr.is_load() {
            continue;
        }
        // SAFETY: 调用者确保 base_addr 区域已映射且足够大
        unsafe {
            let dst = base_addr.add(phdr.offset());
            copy_segment(data, phdr, dst)?;
        }
    }

    Ok(entry)
}

// === ProgramHeader64 结构体（已有定义，此处为参考） ===

/// ELF 64 位程序头（56 字节）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProgramHeader64 {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

pub mod segment_type {
    pub const PT_LOAD: u32 = 1;
}

pub mod segment_flags {
    pub const PF_X: u32 = 1;
    pub const PF_W: u32 = 2;
    pub const PF_R: u32 = 4;
}

impl ProgramHeader64 {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<ProgramHeader64>() {
            return Err(ElfError::TooShort);
        }
        // SAFETY: 长度已检查 >= 56
        let phdr = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ProgramHeader64)
        };
        Ok(phdr)
    }

    pub fn is_load(&self) -> bool { self.p_type == segment_type::PT_LOAD }
    pub fn offset(&self) -> usize { self.p_offset as usize }
    pub fn vaddr(&self) -> usize { self.p_vaddr as usize }
    pub fn file_size(&self) -> usize { self.p_filesz as usize }
    pub fn mem_size(&self) -> usize { self.p_memsz as usize }
    pub fn is_readable(&self) -> bool { self.p_flags & segment_flags::PF_R != 0 }
    pub fn is_writable(&self) -> bool { self.p_flags & segment_flags::PF_W != 0 }
    pub fn is_executable(&self) -> bool { self.p_flags & segment_flags::PF_X != 0 }
}
