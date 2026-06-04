//! # 参考实现：ELF 头解析
//!
//! 本文件展示如何从原始字节解析 ELF64 头并验证其有效性。
//!
//! ## 核心思路
//!
//! ELF 头是文件的前 64 字节，加载器需要：
//! 1. 验证魔数 `0x7f 'E' 'L' 'F'` — 确认是 ELF 文件
//! 2. 检查 ELFCLASS64 + ELFDATA2LSB — 确认是 64 位小端
//! 3. 检查 EM_RISCV — 确认是 RISC-V 程序
//! 4. 检查 ET_EXEC/ET_DYN — 确认是可执行文件
//! 5. 提取入口点和程序头表位置 — 为后续加载做准备

// ELF 常量
pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
pub const EI_CLASS: usize = 4;
pub const EI_DATA: usize = 5;
pub const ELFCLASS64: u8 = 2;
pub const ELFDATA2LSB: u8 = 1;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;
pub const EM_RISCV: u16 = 243;

/// ELF 解析错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ElfError {
    TooShort,
    BadMagic,
    NotElf64,
    NotLittleEndian,
    NotRiscV,
    NotExecutable,
}

/// ELF64 文件头（#[repr(C)] 匹配二进制布局）
#[derive(Debug, Clone, Copy)]
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
    /// 从原始字节读取 ELF 头
    pub fn from_bytes(data: &[u8]) -> Result<Self, ElfError> {
        if data.len() < 64 {
            return Err(ElfError::TooShort);
        }
        // SAFETY: 长度已检查 >= 64，ElfHeader64 是 #[repr(C)] POD 类型
        let header = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ElfHeader64)
        };
        Ok(header)
    }

    /// 验证：魔数 → ELF64 → 小端 → RISC-V → 可执行
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
}

/// 解析并验证 ELF 头，返回关键信息
///
/// 返回 (entry, phoff, phentsize, phnum)
pub fn parse_elf_header(data: &[u8]) -> Result<(u64, u64, u16, u16), ElfError> {
    let header = ElfHeader64::from_bytes(data)?;
    header.validate()?;
    Ok((header.e_entry, header.e_phoff, header.e_phentsize, header.e_phnum))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_valid_elf_bytes() -> [u8; 64] {
        let mut data = [0u8; 64];
        data[0..4].copy_from_slice(&ELF_MAGIC);
        data[4] = ELFCLASS64;
        data[5] = ELFDATA2LSB;
        data[6] = 1;
        data[16..18].copy_from_slice(&ET_EXEC.to_le_bytes());
        data[18..20].copy_from_slice(&EM_RISCV.to_le_bytes());
        data[20..24].copy_from_slice(&1u32.to_le_bytes());
        data[24..32].copy_from_slice(&0x8020_0000u64.to_le_bytes());
        data[32..40].copy_from_slice(&64u64.to_le_bytes());
        data[52..54].copy_from_slice(&64u16.to_le_bytes());
        data[54..56].copy_from_slice(&56u16.to_le_bytes());
        data[56..58].copy_from_slice(&2u16.to_le_bytes());
        data
    }

    #[test]
    fn parse_valid() {
        let data = make_valid_elf_bytes();
        let (entry, phoff, phentsize, phnum) = parse_elf_header(&data).unwrap();
        assert_eq!(entry, 0x8020_0000);
        assert_eq!(phoff, 64);
        assert_eq!(phentsize, 56);
        assert_eq!(phnum, 2);
    }

    #[test]
    fn too_short() {
        assert_eq!(parse_elf_header(&[0u8; 32]), Err(ElfError::TooShort));
    }

    #[test]
    fn bad_magic() {
        let mut data = make_valid_elf_bytes();
        data[0] = 0x00;
        assert_eq!(parse_elf_header(&data), Err(ElfError::BadMagic));
    }

    #[test]
    fn not_elf64() {
        let mut data = make_valid_elf_bytes();
        data[EI_CLASS] = 1; // ELFCLASS32
        assert_eq!(parse_elf_header(&data), Err(ElfError::NotElf64));
    }
}
