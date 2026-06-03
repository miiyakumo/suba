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

/// ELF 64 位文件头
#[derive(Debug, Clone, Copy)]
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
