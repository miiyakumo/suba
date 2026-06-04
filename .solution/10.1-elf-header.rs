// .solution/10.1-elf-header.rs — ELF 头解析参考实现
//
// 本文件是 feature 10.1 的参考实现。
// 学生应在 kernel/src/loader/mod.rs 中实现 ELF 头的解析和验证。
//
// ## 实现要点
//
// 1. ELF 常量定义（EI_CLASS, ELFCLASS64, EM_RISCV, ET_EXEC 等）
// 2. ElfError 错误枚举
// 3. ElfHeader64::from_bytes() — 从原始字节解析 ELF 头
// 4. ElfHeader64::validate() — 验证 ELF64 + 小端 + RISC-V + 可执行
// 5. 入口点和程序头表的提取方法
//
// ## 教学概念：ELF 文件格式
//
// ELF (Executable and Linkable Format) 是 Linux/RISC-V 的标准可执行格式。
// 一个 ELF 文件由以下部分组成：
//
//   ┌──────────────────┐  ← 文件起始
//   │   ELF Header     │  64 字节（ELF64），描述文件整体属性
//   ├──────────────────┤
//   │ Program Headers  │  描述"段"（Segment）— 加载器用这个
//   ├──────────────────┤
//   │   .text 段       │  机器代码
//   │   .rodata 段     │  只读数据
//   │   .data 段       │  已初始化全局变量
//   │   .bss 段        │  未初始化全局变量（文件中不占空间）
//   ├──────────────────┤
//   │ Section Headers  │  描述"节"（Section）— 链接器用这个
//   └──────────────────┘
//
// 加载器只关心 ELF Header 和 Program Headers：
// 1. 读取 ELF Header → 验证魔数、架构、类型
// 2. 读取 Program Headers → 找到 PT_LOAD 段
// 3. 将 PT_LOAD 段复制到内存中 p_vaddr 指定的位置
// 4. 跳转到 e_entry 开始执行

// === ELF 常量 ===

pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

// e_ident 索引
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const EI_VERSION: usize = 6;

// ELFCLASS
pub const ELFCLASS32: u8 = 1;
pub const ELFCLASS64: u8 = 2;

// ELFDATA
pub const ELFDATA2LSB: u8 = 1;
pub const ELFDATA2MSB: u8 = 2;

// e_type
pub const ET_REL: u16 = 1;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;

// e_machine
pub const EM_RISCV: u16 = 243;

// === 错误类型 ===

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    TooShort,
    BadMagic,
    NotElf64,
    NotLittleEndian,
    NotRiscV,
    NotExecutable,
    PhdrOutOfBounds,
}

// === ELF 头结构体 ===

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct ElfHeader64 {
    pub e_ident: [u8; 16],
    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,
    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,
    pub e_flags: u32,
    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,
    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl ElfHeader64 {
    /// 从原始字节解析 ELF 头
    pub fn from_bytes(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < core::mem::size_of::<ElfHeader64>() {
            return Err(ElfError::TooShort);
        }
        // SAFETY: 长度已检查 >= 64，ElfHeader64 是 #[repr(C)] POD 类型
        let header = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ElfHeader64)
        };
        Ok(header)
    }

    /// 验证 ELF 头：魔数 → ELF64 → 小端 → RISC-V → 可执行
    pub fn validate(&self) -> Result<(), ElfError> {
        if self.e_ident[0..4] != ELF_MAGIC {
            return Err(ElfError::BadMagic);
        }
        if self.e_ident[EI_CLASS] != ELFCLASS64 {
            return Err(ElfError::NotElf64);
        }
        if self.e_ident[EI_DATA] != ELFDATA2LSB {
            return Err(ElfError::NotLittleEndian);
        }
        if self.e_machine != EM_RISCV {
            return Err(ElfError::NotRiscV);
        }
        if self.e_type != ET_EXEC && self.e_type != ET_DYN {
            return Err(ElfError::NotExecutable);
        }
        Ok(())
    }

    pub fn entry_point(&self) -> usize {
        self.e_entry as usize
    }

    pub fn phdr_offset(&self) -> usize {
        self.e_phoff as usize
    }

    pub fn phdr_count(&self) -> usize {
        self.e_phnum as usize
    }

    pub fn phdr_entry_size(&self) -> usize {
        self.e_phentsize as usize
    }
}

// === 便捷函数 ===

/// 从原始字节解析并验证 ELF64 RISC-V 头
///
/// 返回 (entry, phoff, phentsize, phnum)
pub fn parse_elf_header(data: &[u8]) -> Result<(u64, u64, u16, u16), ElfError> {
    let header = ElfHeader64::from_bytes(data)?;
    header.validate()?;
    Ok((header.e_entry, header.e_phoff, header.e_phentsize, header.e_phnum))
}
