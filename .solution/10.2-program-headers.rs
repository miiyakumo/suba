//! # 参考实现：程序头解析
//!
//! 本文件展示如何遍历 ELF 程序头表，解析所有 PT_LOAD 段。
//!
//! ## 核心思路
//!
//! 1. 从 ELF 头获取程序头表的位置 (phoff) 和大小 (phentsize * phnum)
//! 2. 逐条读取 56 字节的程序头条目
//! 3. 提取每个段的：类型、权限、文件偏移、虚拟地址、文件大小、内存大小

pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

/// ELF64 程序头（56 字节，#[repr(C)]）
#[derive(Debug, Clone, Copy)]
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

impl ProgramHeader64 {
    pub fn from_bytes(data: &[u8]) -> Result<Self, ()> {
        if data.len() < 56 {
            return Err(());
        }
        // SAFETY: 长度已检查 >= 56，ProgramHeader64 是 #[repr(C)] POD 类型
        let phdr = unsafe {
            core::ptr::read_unaligned(data.as_ptr() as *const ProgramHeader64)
        };
        Ok(phdr)
    }

    pub fn is_load(&self) -> bool {
        self.p_type == PT_LOAD
    }
}

/// 遍历程序头表，解析所有条目
pub fn parse_program_headers(
    data: &[u8],
    phoff: u64,
    phentsize: u16,
    phnum: u16,
) -> Result<alloc::vec::Vec<ProgramHeader64>, ()> {
    let phoff = phoff as usize;
    let entsize = phentsize as usize;
    let count = phnum as usize;

    if entsize < 56 {
        return Err(());
    }
    let table_end = phoff + entsize * count;
    if table_end > data.len() {
        return Err(());
    }

    let mut phdrs = alloc::vec::Vec::with_capacity(count);
    for i in 0..count {
        let offset = phoff + i * entsize;
        let phdr = ProgramHeader64::from_bytes(&data[offset..offset + entsize])?;
        phdrs.push(phdr);
    }
    Ok(phdrs)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 构建一个包含 ELF 头 + 2 个程序头的 mini ELF
    fn make_elf_with_phdrs() -> alloc::vec::Vec<u8> {
        let mut data = alloc::vec![0u8; 64 + 56 * 2];
        // ELF 头
        data[0..4].copy_from_slice(b"\x7fELF");
        data[4] = 2; // ELFCLASS64
        data[5] = 1; // ELFDATA2LSB
        data[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        data[18..20].copy_from_slice(&243u16.to_le_bytes()); // EM_RISCV
        data[32..40].copy_from_slice(&64u64.to_le_bytes()); // phoff
        data[54..56].copy_from_slice(&56u16.to_le_bytes()); // phentsize
        data[56..58].copy_from_slice(&2u16.to_le_bytes());  // phnum

        // phdr 0: .text (R+X)
        let p = 64;
        data[p..p+4].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        data[p+4..p+8].copy_from_slice(&5u32.to_le_bytes()); // PF_R|PF_X
        data[p+16..p+24].copy_from_slice(&0x8020_0000u64.to_le_bytes());
        data[p+32..p+40].copy_from_slice(&0x1000u64.to_le_bytes());

        // phdr 1: .data (R+W)
        let p = 64 + 56;
        data[p..p+4].copy_from_slice(&1u32.to_le_bytes());
        data[p+4..p+8].copy_from_slice(&6u32.to_le_bytes()); // PF_R|PF_W
        data[p+16..p+24].copy_from_slice(&0x8020_2000u64.to_le_bytes());
        data[p+32..p+40].copy_from_slice(&0x100u64.to_le_bytes());
        data[p+40..p+48].copy_from_slice(&0x200u64.to_le_bytes());

        data
    }

    #[test]
    fn parse_two_load_segments() {
        let data = make_elf_with_phdrs();
        let phdrs = parse_program_headers(&data, 64, 56, 2).unwrap();
        assert_eq!(phdrs.len(), 2);
        assert!(phdrs[0].is_load());
        assert!(phdrs[1].is_load());
        assert_eq!(phdrs[0].p_vaddr, 0x8020_0000);
        assert_eq!(phdrs[1].p_vaddr, 0x8020_2000);
    }

    #[test]
    fn out_of_bounds() {
        let data = make_elf_with_phdrs();
        assert!(parse_program_headers(&data, 9999, 56, 1).is_err());
    }
}
