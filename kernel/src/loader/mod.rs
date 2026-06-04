//! # ELF 加载器 (loader)
//!
//! 本模块实现 ELF 格式的用户程序加载。
//!
//! ## 教学概念
//! - **ELF (Executable and Linkable Format)**：Linux/RISC-V 的标准可执行格式
//! - **加载 (Loading)**：将 ELF 文件中的段复制到内存中正确的位置
//! - **入口点 (Entry Point)**：程序开始执行的地址
//! - **段 (Segment)**：ELF 中的代码段、数据段等
//!
//! ## 教学故事
//! "从你敲下 `cargo run` 到用户程序输出 'Hello, world!'，中间发生了什么？"
//! 答案：编译器生成 ELF → 加载器解析 ELF → 将段复制到内存 → 跳转到入口点。

/// ELF 魔数
pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

// ---------------------------------------------------------------------------
// ELF 常量：ident 索引和值
// ---------------------------------------------------------------------------

/// e_ident 数组索引：文件类（32/64 位）
pub const EI_CLASS: usize = 4;
/// e_ident 数组索引：数据编码（大/小端）
pub const EI_DATA: usize = 5;
/// e_ident 数组索引：ELF 版本
pub const EI_VERSION: usize = 6;
/// e_ident 数组索引：OS/ABI
pub const EI_OSABI: usize = 7;

/// ELFCLASS：32 位
pub const ELFCLASS32: u8 = 1;
/// ELFCLASS：64 位
pub const ELFCLASS64: u8 = 2;

/// ELFDATA：小端序
pub const ELFDATA2LSB: u8 = 1;
/// ELFDATA：大端序
pub const ELFDATA2MSB: u8 = 2;

// ---------------------------------------------------------------------------
// ELF 常量：文件类型 (e_type)
// ---------------------------------------------------------------------------

/// 可重定位文件（.o）
pub const ET_REL: u16 = 1;
/// 可执行文件
pub const ET_EXEC: u16 = 2;
/// 共享目标文件（.so / PIE）
pub const ET_DYN: u16 = 3;

// ---------------------------------------------------------------------------
// ELF 常量：机器类型 (e_machine)
// ---------------------------------------------------------------------------

/// RISC-V 架构
pub const EM_RISCV: u16 = 243;

// ---------------------------------------------------------------------------
// ELF 错误类型
// ---------------------------------------------------------------------------

/// ELF 解析错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    /// 数据太短，无法容纳 ELF 头（至少 64 字节）
    TooShort,
    /// 魔数不匹配
    BadMagic,
    /// 不是 64 位 ELF（ELFCLASS != ELFCLASS64）
    NotElf64,
    /// 不是小端序（ELFDATA != ELFDATA2LSB）
    NotLittleEndian,
    /// 不是 RISC-V 架构（e_machine != EM_RISCV）
    NotRiscV,
    /// 不是可执行文件（e_type != ET_EXEC 且 e_type != ET_DYN）
    NotExecutable,
    /// 程序头表偏移或大小超出数据范围
    PhdrOutOfBounds,
}

/// ELF 64 位文件头
///
/// ## 教学概念
///
/// ELF 头是 ELF 文件的前 64 字节，包含：
/// - **魔数** (4 字节)：`0x7f 'E' 'L' 'F'`，标识这是一个 ELF 文件
/// - **文件类** (1 字节)：32 位 (1) 或 64 位 (2)
/// - **数据编码** (1 字节)：小端 (1) 或大端 (2)
/// - **机器类型** (2 字节)：目标 CPU 架构（RISC-V = 243）
/// - **入口点** (8 字节)：程序开始执行的虚拟地址
/// - **程序头表偏移** (8 字节)：段描述表在文件中的位置
///
/// 加载器通过 ELF 头确定：这是给谁的程序？从哪里开始执行？
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ElfHeader64 {
    /// 魔数
    pub e_ident: [u8; 16],
    /// 文件类型
    pub e_type: u16,
    /// 机器类型
    pub e_machine: u16,
    /// ELF 版本
    pub e_version: u32,
    /// 入口点地址
    pub e_entry: u64,
    /// 程序头表偏移
    pub e_phoff: u64,
    /// 节头表偏移
    pub e_shoff: u64,
    /// 标志
    pub e_flags: u32,
    /// ELF 头大小
    pub e_ehsize: u16,
    /// 程序头条目大小
    pub e_phentsize: u16,
    /// 程序头条目数
    pub e_phnum: u16,
    /// 节头条目大小
    pub e_shentsize: u16,
    /// 节头条目数
    pub e_shnum: u16,
    /// 节名字符串表索引
    pub e_shstrndx: u16,
}

impl ElfHeader64 {
    /// 从原始字节解析 ELF 头
    ///
    /// 数据必须至少 64 字节（ELF64 头的大小）。
    ///
    /// # Safety 说明
    ///
    /// 使用 `core::ptr::read_unaligned` 从字节切片读取，
    /// 因为 `ElfHeader64` 是 `#[repr(C)]` 但字节切片不一定对齐。
    ///
    /// # 教学概念
    ///
    /// ELF 文件可能以任意对齐方式出现在内存中（例如嵌入在 initrd 中），
    /// 因此不能直接 cast 指针，需要 unaligned 读取。
    pub fn from_bytes(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<ElfHeader64>() {
            return Err(ElfError::TooShort);
        }
        // SAFETY: data 长度已检查 >= 64 字节，ElfHeader64 是 #[repr(C)] 的 POD 类型
        let header = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ElfHeader64)
        };
        Ok(header)
    }

    /// 验证 ELF 头：魔数 → ELF64 → 小端 → RISC-V → 可执行
    ///
    /// # 教学概念
    ///
    /// 加载器必须严格验证 ELF 头，防止加载损坏或架构不匹配的程序。
    /// 验证顺序从便宜到昂贵：先检查魔数（4 字节比较），
    /// 再检查文件类/编码（单字节），最后检查机器类型（2 字节）。
    pub fn validate(&self) -> Result<(), ElfError> {
        // Step 1: 验证魔数
        if !check_elf_magic(self) {
            return Err(ElfError::BadMagic);
        }
        // Step 2: 验证 64 位
        if self.e_ident[EI_CLASS] != ELFCLASS64 {
            return Err(ElfError::NotElf64);
        }
        // Step 3: 验证小端序
        if self.e_ident[EI_DATA] != ELFDATA2LSB {
            return Err(ElfError::NotLittleEndian);
        }
        // Step 4: 验证 RISC-V 架构
        if self.e_machine != EM_RISCV {
            return Err(ElfError::NotRiscV);
        }
        // Step 5: 验证可执行文件
        if self.e_type != ET_EXEC && self.e_type != ET_DYN {
            return Err(ElfError::NotExecutable);
        }
        Ok(())
    }

    /// 获取入口点地址
    ///
    /// 入口点是 CPU 开始执行用户程序的虚拟地址。
    /// 对于静态链接的程序，通常指向 `_start`；
    /// 对于动态链接的程序，指向动态链接器的入口。
    pub fn entry_point(&self) -> usize {
        self.e_entry as usize
    }

    /// 获取程序头表在文件中的偏移
    ///
    /// 程序头表（Program Header Table）描述了 ELF 文件中的各个段（Segment），
    /// 告诉加载器哪些部分需要被加载到内存的什么位置。
    pub fn phdr_offset(&self) -> usize {
        self.e_phoff as usize
    }

    /// 获取程序头表的条目数量
    pub fn phdr_count(&self) -> usize {
        self.e_phnum as usize
    }

    /// 获取每个程序头条目的大小（字节）
    pub fn phdr_entry_size(&self) -> usize {
        self.e_phentsize as usize
    }
}

/// ELF 64 位程序头
///
/// ## 教学概念
///
/// 程序头（Program Header）描述了一个"段"（Segment），
/// 告诉加载器如何将 ELF 文件的某部分映射到内存：
///
/// ```text
/// p_offset ──→ 文件中的起始位置
/// p_vaddr  ──→ 内存中的目标虚拟地址
/// p_filesz ──→ 从文件复制多少字节
/// p_memsz  ──→ 内存中该段总共占多少字节（>= p_filesz 的部分清零，即 BSS）
/// p_flags  ──→ 权限：可读(R)、可写(W)、可执行(X)
/// ```
///
/// 典型的段布局：
/// - `.text` 段：R+X（代码，只读+可执行）
/// - `.rodata` 段：R（只读数据）
/// - `.data` 段：R+W（已初始化全局变量）
/// - `.bss` 段：R+W（未初始化全局变量，filesz=0, memsz>0）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ProgramHeader64 {
    /// 段类型
    pub p_type: u32,
    /// 标志
    pub p_flags: u32,
    /// 段在文件中的偏移
    pub p_offset: u64,
    /// 段在内存中的虚拟地址
    pub p_vaddr: u64,
    /// 段在内存中的物理地址
    pub p_paddr: u64,
    /// 段在文件中的大小
    pub p_filesz: u64,
    /// 段在内存中的大小
    pub p_memsz: u64,
    /// 对齐
    pub p_align: u64,
}

impl ProgramHeader64 {
    /// 从原始字节解析一个程序头条目
    ///
    /// 数据必须至少 56 字节（ELF64 程序头的大小）。
    ///
    /// # Safety 说明
    ///
    /// 使用 `core::ptr::read_unaligned` 读取，因为字节切片不一定对齐。
    pub fn from_bytes(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<ProgramHeader64>() {
            return Err(ElfError::TooShort);
        }
        // SAFETY: 长度已检查 >= 56，ProgramHeader64 是 #[repr(C)] POD 类型
        let phdr = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ProgramHeader64)
        };
        Ok(phdr)
    }

    /// 此段是否为 PT_LOAD（需要加载到内存的段）
    pub fn is_load(&self) -> bool {
        self.p_type == segment_type::PT_LOAD
    }

    /// 段在文件中的偏移
    pub fn offset(&self) -> usize {
        self.p_offset as usize
    }

    /// 段在内存中的虚拟地址
    pub fn vaddr(&self) -> usize {
        self.p_vaddr as usize
    }

    /// 从文件中复制的字节数
    pub fn file_size(&self) -> usize {
        self.p_filesz as usize
    }

    /// 段在内存中的总大小（>= file_size 的部分由加载器清零）
    pub fn mem_size(&self) -> usize {
        self.p_memsz as usize
    }

    /// 段是否可读
    pub fn is_readable(&self) -> bool {
        self.p_flags & segment_flags::PF_R != 0
    }

    /// 段是否可写
    pub fn is_writable(&self) -> bool {
        self.p_flags & segment_flags::PF_W != 0
    }

    /// 段是否可执行
    pub fn is_executable(&self) -> bool {
        self.p_flags & segment_flags::PF_X != 0
    }
}

/// 程序头类型
pub mod segment_type {
    /// 可加载段（必须加载到内存才能执行）
    pub const PT_LOAD: u32 = 1;
    /// 动态链接信息
    pub const PT_DYNAMIC: u32 = 2;
    /// 解释器路径（动态链接器）
    pub const PT_INTERP: u32 = 3;
}

/// 段权限标志（p_flags）
pub mod segment_flags {
    /// 可执行
    pub const PF_X: u32 = 1;
    /// 可写
    pub const PF_W: u32 = 2;
    /// 可读
    pub const PF_R: u32 = 4;
}

/// ELF 加载器 trait。
///
/// 定义加载用户程序的接口。
pub trait ElfLoader {
    /// 从数据中加载 ELF 程序
    ///
    /// # 返回值
    /// 成功返回 (入口点地址, 用户栈顶地址)，失败返回 Err。
    fn load_elf(data: &[u8]) -> Result<(usize, usize), ()>;
}

/// 验证 ELF 魔数
pub fn check_elf_magic(header: &ElfHeader64) -> bool {
    header.e_ident[0..4] == ELF_MAGIC
}

/// 从原始字节解析并验证 ELF64 RISC-V 头
///
/// 这是加载器的第一步：读取 ELF 文件的前 64 字节，
/// 依次验证魔数、64 位、小端序、RISC-V 架构、可执行文件，
/// 最后提取入口点地址和程序头表位置。
///
/// # 教学概念
///
/// 加载器需要从 ELF 文件中回答两个关键问题：
/// 1. **从哪里开始执行？** → `entry`（入口点地址）
/// 2. **段在哪里？** → `phoff`/`phnum`/`phentsize`（程序头表位置）
///
/// 验证顺序从便宜到昂贵：先检查魔数（4 字节），
/// 再检查文件类/编码（单字节），最后检查机器类型（2 字节）。
///
/// # 参数
/// - `data`：ELF 文件的原始字节
///
/// # 返回值
/// 成功返回 `(entry, phoff, phentsize, phnum)` = (入口点, 程序头表偏移, 条目大小, 条目数)。
pub fn parse_elf_header(data: &[u8]) -> Result<(u64, u64, u16, u16), ElfError> {
    let header = ElfHeader64::from_bytes(data)?;
    header.validate()?;
    Ok((header.e_entry, header.e_phoff, header.e_phentsize, header.e_phnum))
}

/// 从 ELF 数据中解析所有程序头
///
/// 遍历程序头表，解析每个条目并收集到 `Vec` 中。
/// 调用前应先调用 [`parse_elf_header`] 获取 `phoff`/`phentsize`/`phnum`。
///
/// # 教学概念
///
/// 程序头表（Program Header Table）是 ELF 文件中"段"的目录。
/// 加载器遍历这个表，找到所有 `PT_LOAD` 类型的段，
/// 然后将它们从文件复制到内存中正确的虚拟地址。
///
/// # 参数
/// - `data`：ELF 文件的原始字节
/// - `phoff`：程序头表在文件中的偏移（来自 ELF 头）
/// - `phentsize`：每个条目的大小（字节）
/// - `phnum`：条目数量
pub fn parse_program_headers(
    data: &[u8],
    phoff: u64,
    phentsize: u16,
    phnum: u16,
) -> Result<alloc::vec::Vec<ProgramHeader64>, ElfError> {
    let phoff = phoff as usize;
    let entsize = phentsize as usize;
    let count = phnum as usize;

    // 验证条目大小（ELF64 程序头固定 56 字节）
    if entsize < core::mem::size_of::<ProgramHeader64>() {
        return Err(ElfError::PhdrOutOfBounds);
    }

    // 验证整个程序头表在数据范围内
    let table_end = phoff
        .checked_add(entsize.checked_mul(count).ok_or(ElfError::PhdrOutOfBounds)?)
        .ok_or(ElfError::PhdrOutOfBounds)?;
    if table_end > data.len() {
        return Err(ElfError::PhdrOutOfBounds);
    }

    let mut phdrs = alloc::vec::Vec::with_capacity(count);
    for i in 0..count {
        let offset = phoff + i * entsize;
        let phdr = ProgramHeader64::from_bytes(&data[offset..offset + entsize])?;
        phdrs.push(phdr);
    }
    Ok(phdrs)
}

/// 将 PT_LOAD 段从 ELF 数据复制到目标内存地址
///
/// # 教学概念：加载段到内存
///
/// 加载器的核心工作就是将 ELF 中的 PT_LOAD 段复制到内存中正确的位置。
/// 过程如下：
///
/// ```text
/// ELF 文件                     内存 (p_vaddr)
/// ┌──────────────┐            ┌──────────────┐
/// │ p_offset     │  ──copy──→ │ p_vaddr      │  p_filesz 字节
/// │ (p_filesz)   │            │              │
/// └──────────────┘            ├──────────────┤
///                             │  BSS 区域    │  (p_memsz - p_filesz) 字节清零
///                             │  (零填充)    │
///                             └──────────────┘
/// ```
///
/// 当 `p_memsz > p_filesz` 时，多出的部分是 BSS 段（未初始化全局变量），
/// 加载器必须将其清零。这是 C 语言规范的要求：
/// 未初始化的全局变量默认为 0。
///
/// # Safety
///
/// `dst` 必须指向至少 `p_memsz` 字节的可写内存。
/// 调用者负责确保目标地址已经通过页表映射到正确的物理帧。
///
/// # 参数
/// - `data`：ELF 文件的原始字节
/// - `phdr`：程序头（描述要加载的段）
/// - `dst`：目标内存地址（虚拟地址，已经映射）
///
/// # 返回值
/// 成功返回 Ok(())，段数据超出 ELF 文件范围时返回 Err。
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
    //
    // 这包含了程序的代码（.text）、只读数据（.rodata）、
    // 已初始化全局变量（.data）等。
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
    //
    // BSS 段在 ELF 文件中不占空间（p_filesz = 0 或 < p_memsz），
    // 但在内存中需要占空间（p_memsz），且必须初始化为零。
    if memsz > filesz {
        let bss_size = memsz - filesz;
        // SAFETY: 调用者确保 dst 指向至少 memsz 字节的可写内存，
        // 因此 dst.add(filesz) 到 dst.add(memsz) 也是有效的可写范围。
        unsafe {
            let bss_start = dst.add(filesz);
            core::ptr::write_bytes(bss_start, 0, bss_size);
        }
    }

    Ok(())
}

/// 从 ELF 数据加载所有 PT_LOAD 段到目标内存
///
/// 这是加载器的高层接口：解析 ELF → 遍历程序头 → 复制 PT_LOAD 段。
///
/// # Safety
///
/// `base_addr` 必须指向已经映射好的可写内存区域，
/// 大小足以容纳所有 PT_LOAD 段（通常由页表分配器保证）。
///
/// # 返回值
/// 成功返回入口点地址，失败返回 Err。
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
        // 计算段在目标内存中的位置
        // 假设 base_addr 对应 ELF 的第一个 PT_LOAD 段的 vaddr
        // 实际实现中需要根据 phdr.vaddr() 计算偏移
        // SAFETY: 调用者确保 base_addr 区域已映射且足够大
        unsafe {
            let dst = base_addr.add(phdr.offset());
            copy_segment(data, phdr, dst)?;
        }
    }

    Ok(entry)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 创建一个有效的 ELF64 RISC-V 可执行头用于测试
    fn make_valid_header() -> ElfHeader64 {
        let mut h = ElfHeader64 {
            e_ident: [0u8; 16],
            e_type: ET_EXEC,
            e_machine: EM_RISCV,
            e_version: 1,
            e_entry: 0x8020_0000,
            e_phoff: 64,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: 64,
            e_phentsize: 56,
            e_phnum: 1,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };
        // 设置魔数
        h.e_ident[0..4].copy_from_slice(&ELF_MAGIC);
        // ELFCLASS64
        h.e_ident[EI_CLASS] = ELFCLASS64;
        // 小端序
        h.e_ident[EI_DATA] = ELFDATA2LSB;
        // ELF 版本
        h.e_ident[EI_VERSION] = 1;
        h
    }

    #[test]
    fn elf_magic_check() {
        let mut header = ElfHeader64 {
            e_ident: [0u8; 16],
            e_type: 0,
            e_machine: 0,
            e_version: 0,
            e_entry: 0,
            e_phoff: 0,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: 0,
            e_phentsize: 0,
            e_phnum: 0,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };

        // 无效魔数
        assert!(!check_elf_magic(&header));

        // 有效魔数
        header.e_ident[0..4].copy_from_slice(&ELF_MAGIC);
        assert!(check_elf_magic(&header));
    }

    #[test]
    fn elf_header_size() {
        // ELF64 头应该是 64 字节
        assert_eq!(core::mem::size_of::<ElfHeader64>(), 64);
    }

    #[test]
    fn program_header_size() {
        // ELF64 程序头应该是 56 字节
        assert_eq!(core::mem::size_of::<ProgramHeader64>(), 56);
    }

    #[test]
    fn from_bytes_valid() {
        let header = make_valid_header();
        // SAFETY: ElfHeader64 是 #[repr(C)] POD 类型
        let bytes = unsafe {
            core::slice::from_raw_parts(
                &header as *const _ as *const u8,
                core::mem::size_of::<ElfHeader64>(),
            )
        };
        let parsed = ElfHeader64::from_bytes(bytes).unwrap();
        assert_eq!(parsed.e_entry, 0x8020_0000);
        assert_eq!(parsed.e_machine, EM_RISCV);
    }

    #[test]
    fn from_bytes_too_short() {
        let short_data = [0u8; 32];
        assert_eq!(ElfHeader64::from_bytes(&short_data), Err(ElfError::TooShort));
    }

    #[test]
    fn validate_valid_header() {
        let header = make_valid_header();
        assert!(header.validate().is_ok());
    }

    #[test]
    fn validate_bad_magic() {
        let mut header = make_valid_header();
        header.e_ident[0] = 0x00;
        assert_eq!(header.validate(), Err(ElfError::BadMagic));
    }

    #[test]
    fn validate_not_elf64() {
        let mut header = make_valid_header();
        header.e_ident[EI_CLASS] = ELFCLASS32;
        assert_eq!(header.validate(), Err(ElfError::NotElf64));
    }

    #[test]
    fn validate_not_little_endian() {
        let mut header = make_valid_header();
        header.e_ident[EI_DATA] = ELFDATA2MSB;
        assert_eq!(header.validate(), Err(ElfError::NotLittleEndian));
    }

    #[test]
    fn validate_not_riscv() {
        let mut header = make_valid_header();
        header.e_machine = 0x03; // x86 (EM_386)
        assert_eq!(header.validate(), Err(ElfError::NotRiscV));
    }

    #[test]
    fn validate_not_executable() {
        let mut header = make_valid_header();
        header.e_type = ET_REL;
        assert_eq!(header.validate(), Err(ElfError::NotExecutable));
    }

    #[test]
    fn validate_pie_accepted() {
        // ET_DYN（PIE）也应该通过验证
        let mut header = make_valid_header();
        header.e_type = ET_DYN;
        assert!(header.validate().is_ok());
    }

    #[test]
    fn extract_entry_point() {
        let header = make_valid_header();
        assert_eq!(header.entry_point(), 0x8020_0000);
    }

    #[test]
    fn extract_phdr_info() {
        let header = make_valid_header();
        assert_eq!(header.phdr_offset(), 64);
        assert_eq!(header.phdr_count(), 1);
        assert_eq!(header.phdr_entry_size(), 56);
    }

    /// 构建一个包含有效 ELF 头的 64 字节数组（小端序）
    fn make_valid_elf_bytes() -> [u8; 64] {
        let mut data = [0u8; 64];
        // e_ident[0..4] = 魔数
        data[0..4].copy_from_slice(&ELF_MAGIC);
        // e_ident[4] = ELFCLASS64
        data[4] = ELFCLASS64;
        // e_ident[5] = ELFDATA2LSB
        data[5] = ELFDATA2LSB;
        // e_ident[6] = ELF 版本
        data[6] = 1;
        // e_type = ET_EXEC (偏移 16)
        data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        // e_machine = EM_RISCV (偏移 18)
        data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        // e_version = 1 (偏移 20)
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        // e_entry = 0x80200000 (偏移 24)
        data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes());
        // e_phoff = 64 (偏移 32)
        data[32..40].copy_from_slice(&64u64.to_le_bytes());
        // e_ehsize = 64 (偏移 52)
        data[52..54].copy_from_slice(&64u16.to_le_bytes());
        // e_phentsize = 56 (偏移 54)
        data[54..56].copy_from_slice(&56u16.to_le_bytes());
        // e_phnum = 2 (偏移 56)
        data[56..58].copy_from_slice(&2u16.to_le_bytes());
        data
    }

    #[test]
    fn parse_elf_header_valid() {
        let data = make_valid_elf_bytes();
        let (entry, phoff, phentsize, phnum) = parse_elf_header(&data).unwrap();
        assert_eq!(entry, 0x8020_0000);
        assert_eq!(phoff, 64);
        assert_eq!(phentsize, 56);
        assert_eq!(phnum, 2);
    }

    #[test]
    fn parse_elf_header_too_short() {
        let data = [0u8; 32];
        assert_eq!(parse_elf_header(&data), Err(ElfError::TooShort));
    }

    #[test]
    fn parse_elf_header_bad_magic() {
        let mut data = make_valid_elf_bytes();
        data[0] = 0x00;
        assert_eq!(parse_elf_header(&data), Err(ElfError::BadMagic));
    }

    #[test]
    fn parse_elf_header_not_elf64() {
        let mut data = make_valid_elf_bytes();
        data[EI_CLASS] = ELFCLASS32;
        assert_eq!(parse_elf_header(&data), Err(ElfError::NotElf64));
    }

    #[test]
    fn parse_elf_header_not_little_endian() {
        let mut data = make_valid_elf_bytes();
        data[EI_DATA] = ELFDATA2MSB;
        assert_eq!(parse_elf_header(&data), Err(ElfError::NotLittleEndian));
    }

    #[test]
    fn parse_elf_header_not_riscv() {
        let mut data = make_valid_elf_bytes();
        data[18..20].copy_from_slice(&3u16.to_le_bytes()); // EM_386
        assert_eq!(parse_elf_header(&data), Err(ElfError::NotRiscV));
    }

    #[test]
    fn parse_elf_header_not_executable() {
        let mut data = make_valid_elf_bytes();
        data[16..18].copy_from_slice(&ET_REL.to_le_bytes());
        assert_eq!(parse_elf_header(&data), Err(ElfError::NotExecutable));
    }

    #[test]
    fn parse_elf_header_pie() {
        let mut data = make_valid_elf_bytes();
        data[16..18].copy_from_slice(&ET_DYN.to_le_bytes());
        let result = parse_elf_header(&data);
        assert!(result.is_ok());
    }

    // -----------------------------------------------------------------------
    // 程序头解析测试
    // -----------------------------------------------------------------------

    /// 构建一个包含 ELF 头 + 2 个程序头的 mini ELF 文件
    ///
    /// 布局：[64 字节 ELF 头][56 字节 phdr 0 (text)][56 字节 phdr 1 (data)]
    fn make_elf_with_phdrs() -> alloc::vec::Vec<u8> {
        let mut data = alloc::vec![0u8; 64 + 56 * 2];

        // ELF 头
        data[0..4].copy_from_slice(&ELF_MAGIC);
        data[4] = ELFCLASS64;
        data[5] = ELFDATA2LSB;
        data[6] = 1;
        data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes()); // entry
        data[32..40].copy_from_slice(&64u64.to_le_bytes());           // phoff
        data[52..54].copy_from_slice(&64u16.to_le_bytes());           // e_ehsize
        data[54..56].copy_from_slice(&56u16.to_le_bytes());           // phentsize
        data[56..58].copy_from_slice(&2u16.to_le_bytes());            // phnum

        // phdr 0: .text 段 (PT_LOAD, R+X)
        let p0 = 64;
        data[p0..p0+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        data[p0+4..p0+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_X).to_le_bytes());
        data[p0+8..p0+16].copy_from_slice(&0u64.to_le_bytes());           // p_offset
        data[p0+16..p0+24].copy_from_slice(&0x8020_0000u64.to_le_bytes()); // p_vaddr
        data[p0+32..p0+40].copy_from_slice(&0x1000u64.to_le_bytes());      // p_filesz
        data[p0+40..p0+48].copy_from_slice(&0x1000u64.to_le_bytes());      // p_memsz

        // phdr 1: .data 段 (PT_LOAD, R+W)
        let p1 = 64 + 56;
        data[p1..p1+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        data[p1+4..p1+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_W).to_le_bytes());
        data[p1+8..p1+16].copy_from_slice(&0x2000u64.to_le_bytes());       // p_offset
        data[p1+16..p1+24].copy_from_slice(&0x8020_2000u64.to_le_bytes());  // p_vaddr
        data[p1+32..p1+40].copy_from_slice(&0x100u64.to_le_bytes());        // p_filesz
        data[p1+40..p1+48].copy_from_slice(&0x200u64.to_le_bytes());        // p_memsz

        data
    }

    #[test]
    fn parse_program_headers_valid() {
        let data = make_elf_with_phdrs();
        let phdrs = parse_program_headers(&data, 64, 56, 2).unwrap();
        assert_eq!(phdrs.len(), 2);

        // .text 段
        assert_eq!(phdrs[0].p_type, segment_type::PT_LOAD);
        assert_eq!(phdrs[0].p_vaddr, 0x8020_0000);
        assert_eq!(phdrs[0].p_filesz, 0x1000);
        assert_eq!(phdrs[0].p_memsz, 0x1000);
        assert!(phdrs[0].is_executable());
        assert!(!phdrs[0].is_writable());

        // .data 段
        assert_eq!(phdrs[1].p_type, segment_type::PT_LOAD);
        assert_eq!(phdrs[1].p_vaddr, 0x8020_2000);
        assert_eq!(phdrs[1].p_filesz, 0x100);
        assert_eq!(phdrs[1].p_memsz, 0x200);
        assert!(phdrs[1].is_writable());
        assert!(!phdrs[1].is_executable());
    }

    #[test]
    fn parse_program_headers_out_of_bounds() {
        let data = make_elf_with_phdrs();
        // phoff 超出数据范围
        assert_eq!(parse_program_headers(&data, 9999, 56, 1), Err(ElfError::PhdrOutOfBounds));
    }

    #[test]
    fn parse_program_headers_bad_entsize() {
        let data = make_elf_with_phdrs();
        // entsize 小于 56
        assert_eq!(parse_program_headers(&data, 64, 32, 1), Err(ElfError::PhdrOutOfBounds));
    }

    #[test]
    fn program_header_from_bytes() {
        let mut buf = [0u8; 56];
        // PT_LOAD, flags=R+X, offset=0, vaddr=0x10000, filesz=0x100, memsz=0x200
        buf[0..4].copy_from_slice(&1u32.to_le_bytes());     // PT_LOAD
        buf[4..8].copy_from_slice(&5u32.to_le_bytes());     // PF_R | PF_X
        buf[8..16].copy_from_slice(&0u64.to_le_bytes());     // p_offset
        buf[16..24].copy_from_slice(&0x10000u64.to_le_bytes()); // p_vaddr
        buf[32..40].copy_from_slice(&0x100u64.to_le_bytes());   // p_filesz
        buf[40..48].copy_from_slice(&0x200u64.to_le_bytes());   // p_memsz

        let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
        assert!(phdr.is_load());
        assert!(phdr.is_readable());
        assert!(phdr.is_executable());
        assert!(!phdr.is_writable());
        assert_eq!(phdr.vaddr(), 0x10000);
        assert_eq!(phdr.file_size(), 0x100);
        assert_eq!(phdr.mem_size(), 0x200);
    }

    #[test]
    fn program_header_from_bytes_too_short() {
        assert!(ProgramHeader64::from_bytes(&[0u8; 20]).is_err());
    }

    // -----------------------------------------------------------------------
    // copy_segment 测试
    // -----------------------------------------------------------------------

    #[test]
    fn copy_segment_basic() {
        // 构建一个包含 16 字节数据的 ELF
        let mut elf_data = alloc::vec![0xABu8; 176]; // 64 (ELF header) + 56 (phdr) + 16 (data) + padding
        // 设置 ELF 头
        elf_data[0..4].copy_from_slice(&ELF_MAGIC);
        elf_data[4] = ELFCLASS64;
        elf_data[5] = ELFDATA2LSB;
        elf_data[6] = 1;
        elf_data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        elf_data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        elf_data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes());
        elf_data[32..40].copy_from_slice(&64u64.to_le_bytes());
        elf_data[52..54].copy_from_slice(&64u16.to_le_bytes());
        elf_data[54..56].copy_from_slice(&56u16.to_le_bytes());
        elf_data[56..58].copy_from_slice(&1u16.to_le_bytes());

        // 程序头: PT_LOAD, offset=64+56=120, filesz=16, memsz=32
        let p0 = 64;
        elf_data[p0..p0+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        elf_data[p0+4..p0+8].copy_from_slice(&segment_flags::PF_R.to_le_bytes());
        elf_data[p0+8..p0+16].copy_from_slice(&120u64.to_le_bytes()); // p_offset
        elf_data[p0+32..p0+40].copy_from_slice(&16u64.to_le_bytes()); // p_filesz
        elf_data[p0+40..p0+48].copy_from_slice(&32u64.to_le_bytes()); // p_memsz

        // 在 offset 120 写入测试数据
        for i in 0..16 {
            elf_data[120 + i] = (i + 1) as u8;
        }

        // 目标缓冲区
        let mut dst = [0u8; 32];
        let phdr = ProgramHeader64::from_bytes(&elf_data[64..120]).unwrap();

        // SAFETY: dst 足够大（32 字节）
        unsafe {
            copy_segment(&elf_data, &phdr, dst.as_mut_ptr()).unwrap();
        }

        // 验证：前 16 字节是从 ELF 复制的数据
        for i in 0..16 {
            assert_eq!(dst[i], (i + 1) as u8, "byte {} should be copied", i);
        }
        // 验证：后 16 字节是 BSS 清零
        for i in 16..32 {
            assert_eq!(dst[i], 0, "BSS byte {} should be zero", i);
        }
    }

    #[test]
    fn copy_segment_out_of_bounds() {
        // 声称 filesz 很大，但 ELF 数据不够
        let mut elf_data = alloc::vec![0u8; 120];
        elf_data[0..4].copy_from_slice(&ELF_MAGIC);
        elf_data[4] = ELFCLASS64;
        elf_data[5] = ELFDATA2LSB;

        let p0 = 64;
        elf_data[p0..p0+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        elf_data[p0+8..p0+16].copy_from_slice(&0u64.to_le_bytes()); // p_offset
        elf_data[p0+32..p0+40].copy_from_slice(&9999u64.to_le_bytes()); // p_filesz too large
        elf_data[p0+40..p0+48].copy_from_slice(&9999u64.to_le_bytes()); // p_memsz

        let phdr = ProgramHeader64::from_bytes(&elf_data[64..120]).unwrap();
        let mut dst = [0u8; 16];
        // SAFETY: dst 足够大
        let result = unsafe { copy_segment(&elf_data, &phdr, dst.as_mut_ptr()) };
        assert_eq!(result, Err(ElfError::PhdrOutOfBounds));
    }

    // -----------------------------------------------------------------------
    // 边界条件测试
    // -----------------------------------------------------------------------

    /// 构建一个包含 3 个程序头的 mini ELF
    ///
    /// phdr 0: .text (PT_LOAD, R+X, filesz=8, memsz=8)
    /// phdr 1: PT_DYNAMIC (非 PT_LOAD，应被跳过)
    /// phdr 2: .bss (PT_LOAD, R+W, filesz=0, memsz=16)
    fn make_elf_with_mixed_phdrs() -> alloc::vec::Vec<u8> {
        let phdr_count = 3u16;
        let data_size = 64 + 56 * phdr_count as usize + 8; // +8 for .text data
        let mut data = alloc::vec![0u8; data_size];

        // ELF 头
        data[0..4].copy_from_slice(&ELF_MAGIC);
        data[4] = ELFCLASS64;
        data[5] = ELFDATA2LSB;
        data[6] = 1;
        data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        data[24..32].copy_from_slice(&0x10000u64.to_le_bytes()); // entry
        data[32..40].copy_from_slice(&64u64.to_le_bytes());      // phoff
        data[52..54].copy_from_slice(&64u16.to_le_bytes());      // e_ehsize
        data[54..56].copy_from_slice(&56u16.to_le_bytes());      // phentsize
        data[56..58].copy_from_slice(&phdr_count.to_le_bytes()); // phnum

        // 在 offset 176 (= 64 + 56*2) 放 .text 数据（8 字节）
        let text_data_offset = 64 + 56 * phdr_count as usize;
        for i in 0..8 {
            data[text_data_offset + i] = (0xAA + i) as u8;
        }

        // phdr 0: .text (PT_LOAD, R+X, filesz=8, memsz=8)
        let p0 = 64;
        data[p0..p0+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        data[p0+4..p0+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_X).to_le_bytes());
        data[p0+8..p0+16].copy_from_slice(&(text_data_offset as u64).to_le_bytes()); // p_offset
        data[p0+16..p0+24].copy_from_slice(&0x10000u64.to_le_bytes()); // p_vaddr
        data[p0+32..p0+40].copy_from_slice(&8u64.to_le_bytes());       // p_filesz
        data[p0+40..p0+48].copy_from_slice(&8u64.to_le_bytes());       // p_memsz

        // phdr 1: PT_DYNAMIC (非 PT_LOAD，应被跳过)
        let p1 = 64 + 56;
        data[p1..p1+4].copy_from_slice(&segment_type::PT_DYNAMIC.to_le_bytes());
        data[p1+4..p1+8].copy_from_slice(&segment_flags::PF_R.to_le_bytes());

        // phdr 2: .bss (PT_LOAD, R+W, filesz=0, memsz=16)
        let p2 = 64 + 56 * 2;
        data[p2..p2+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        data[p2+4..p2+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_W).to_le_bytes());
        data[p2+16..p2+24].copy_from_slice(&0x20000u64.to_le_bytes()); // p_vaddr
        data[p2+32..p2+40].copy_from_slice(&0u64.to_le_bytes());       // p_filesz (纯 BSS)
        data[p2+40..p2+48].copy_from_slice(&16u64.to_le_bytes());      // p_memsz

        data
    }

    #[test]
    fn parse_program_headers_mixed_types() {
        // 应解析出全部 3 个程序头（包括非 PT_LOAD 的）
        let data = make_elf_with_mixed_phdrs();
        let phdrs = parse_program_headers(&data, 64, 56, 3).unwrap();
        assert_eq!(phdrs.len(), 3);

        assert!(phdrs[0].is_load()); // .text
        assert!(!phdrs[1].is_load()); // PT_DYNAMIC
        assert!(phdrs[2].is_load()); // .bss
    }

    #[test]
    fn copy_segment_bss_only() {
        // 纯 BSS 段：filesz=0, memsz=16
        let mut buf = [0u8; 56];
        buf[0..4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        buf[4..8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_W).to_le_bytes());
        buf[32..40].copy_from_slice(&0u64.to_le_bytes());  // p_filesz = 0
        buf[40..48].copy_from_slice(&16u64.to_le_bytes());  // p_memsz = 16

        let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
        let mut dst = [0xFFu8; 16]; // 预填充非零值

        // SAFETY: dst 足够大
        unsafe {
            copy_segment(&[], &phdr, dst.as_mut_ptr()).unwrap();
        }

        // BSS 段应被清零
        for i in 0..16 {
            assert_eq!(dst[i], 0, "BSS byte {} should be zero", i);
        }
    }

    #[test]
    fn copy_segment_zero_memsz() {
        // filesz=0, memsz=0：空段，应成功但什么都不做
        let mut buf = [0u8; 56];
        buf[0..4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        buf[32..40].copy_from_slice(&0u64.to_le_bytes()); // p_filesz = 0
        buf[40..48].copy_from_slice(&0u64.to_le_bytes()); // p_memsz = 0

        let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
        let mut dst = [0xABu8; 4];

        // SAFETY: dst 存在
        unsafe {
            copy_segment(&[], &phdr, dst.as_mut_ptr()).unwrap();
        }

        // dst 应保持不变
        assert_eq!(dst, [0xABu8; 4]);
    }

    #[test]
    fn parse_program_headers_zero_count() {
        // phnum=0：空程序头表，应返回空 Vec
        let data = make_valid_elf_bytes();
        let phdrs = parse_program_headers(&data, 64, 56, 0).unwrap();
        assert_eq!(phdrs.len(), 0);
    }

    #[test]
    fn parse_program_headers_larger_entsize() {
        // entsize > 56（ELF 允许条目大于最小值）
        let data = make_elf_with_phdrs();
        // entsize=64 > 56，应该能解析（跳过额外字节）
        let phdrs = parse_program_headers(&data, 64, 64, 1).unwrap();
        assert_eq!(phdrs.len(), 1);
        assert_eq!(phdrs[0].p_type, segment_type::PT_LOAD);
    }

    #[test]
    fn program_header_flags_all_combinations() {
        let test_cases = [
            (segment_flags::PF_R, true, false, false),
            (segment_flags::PF_W, false, true, false),
            (segment_flags::PF_X, false, false, true),
            (segment_flags::PF_R | segment_flags::PF_W, true, true, false),
            (segment_flags::PF_R | segment_flags::PF_X, true, false, true),
            (segment_flags::PF_R | segment_flags::PF_W | segment_flags::PF_X, true, true, true),
        ];

        for (flags, expect_r, expect_w, expect_x) in test_cases {
            let mut buf = [0u8; 56];
            buf[0..4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
            buf[4..8].copy_from_slice(&flags.to_le_bytes());

            let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
            assert_eq!(phdr.is_readable(), expect_r, "PF_R for flags={:#x}", flags);
            assert_eq!(phdr.is_writable(), expect_w, "PF_W for flags={:#x}", flags);
            assert_eq!(phdr.is_executable(), expect_x, "PF_X for flags={:#x}", flags);
        }
    }

    #[test]
    fn program_header_non_load_types() {
        let types = [
            segment_type::PT_DYNAMIC,
            segment_type::PT_INTERP,
            0x6474e550, // PT_GNU_RELRO
            0x6474e551, // PT_GNU_STACK
        ];

        for ptype in types {
            let mut buf = [0u8; 56];
            buf[0..4].copy_from_slice(&ptype.to_le_bytes());
            let phdr = ProgramHeader64::from_bytes(&buf).unwrap();
            assert!(!phdr.is_load(), "ptype {:#x} should not be PT_LOAD", ptype);
        }
    }

    #[test]
    fn load_elf_segments_with_two_segments() {
        // 构建一个包含 .text + .data 的 ELF
        let text_data_offset: usize = 64 + 56 * 2; // ELF header + 2 phdrs
        let text_data_size: usize = 16;
        let data_data_offset: usize = text_data_offset + text_data_size;
        let data_data_size: usize = 8;
        let total_size = data_data_offset + data_data_size;

        let mut elf_data = alloc::vec![0u8; total_size];

        // ELF 头
        elf_data[0..4].copy_from_slice(&ELF_MAGIC);
        elf_data[4] = ELFCLASS64;
        elf_data[5] = ELFDATA2LSB;
        elf_data[6] = 1;
        elf_data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        elf_data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        elf_data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes()); // entry
        elf_data[32..40].copy_from_slice(&64u64.to_le_bytes());           // phoff
        elf_data[52..54].copy_from_slice(&64u16.to_le_bytes());           // e_ehsize
        elf_data[54..56].copy_from_slice(&56u16.to_le_bytes());           // phentsize
        elf_data[56..58].copy_from_slice(&2u16.to_le_bytes());            // phnum

        // phdr 0: .text (PT_LOAD, R+X, filesz=16, memsz=16)
        let p0 = 64;
        elf_data[p0..p0+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        elf_data[p0+4..p0+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_X).to_le_bytes());
        elf_data[p0+8..p0+16].copy_from_slice(&(text_data_offset as u64).to_le_bytes());
        elf_data[p0+16..p0+24].copy_from_slice(&0x10000u64.to_le_bytes()); // p_vaddr
        elf_data[p0+32..p0+40].copy_from_slice(&(text_data_size as u64).to_le_bytes());
        elf_data[p0+40..p0+48].copy_from_slice(&(text_data_size as u64).to_le_bytes());

        // phdr 1: .data (PT_LOAD, R+W, filesz=8, memsz=16)
        let p1 = 64 + 56;
        elf_data[p1..p1+4].copy_from_slice(&segment_type::PT_LOAD.to_le_bytes());
        elf_data[p1+4..p1+8].copy_from_slice(&(segment_flags::PF_R | segment_flags::PF_W).to_le_bytes());
        elf_data[p1+8..p1+16].copy_from_slice(&(data_data_offset as u64).to_le_bytes());
        elf_data[p1+16..p1+24].copy_from_slice(&0x20000u64.to_le_bytes()); // p_vaddr
        elf_data[p1+32..p1+40].copy_from_slice(&(data_data_size as u64).to_le_bytes());
        elf_data[p1+40..p1+48].copy_from_slice(&16u64.to_le_bytes()); // memsz=16 (BSS)

        // 填充 .text 数据
        for i in 0..text_data_size {
            elf_data[text_data_offset + i] = (i + 1) as u8;
        }
        // 填充 .data 数据
        for i in 0..data_data_size {
            elf_data[data_data_offset + i] = (0x10 + i) as u8;
        }

        // 目标缓冲区
        let mut text_dst = [0u8; 16];
        let mut data_dst = [0u8; 16];

        let phdrs = parse_program_headers(&elf_data, 64, 56, 2).unwrap();

        // 分别加载两个段
        // SAFETY: dst 足够大
        unsafe {
            copy_segment(&elf_data, &phdrs[0], text_dst.as_mut_ptr()).unwrap();
            copy_segment(&elf_data, &phdrs[1], data_dst.as_mut_ptr()).unwrap();
        }

        // 验证 .text 段内容
        for i in 0..text_data_size {
            assert_eq!(text_dst[i], (i + 1) as u8, ".text byte {}", i);
        }

        // 验证 .data 段内容：前 8 字节是文件数据，后 8 字节是 BSS 清零
        for i in 0..data_data_size {
            assert_eq!(data_dst[i], (0x10 + i) as u8, ".data byte {}", i);
        }
        for i in data_data_size..16 {
            assert_eq!(data_dst[i], 0, ".bss byte {} should be zero", i);
        }
    }
}
