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
#[derive(Debug, Clone, Copy)]
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

/// 程序头类型
pub mod segment_type {
    /// 可加载段
    pub const PT_LOAD: u32 = 1;
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
}
