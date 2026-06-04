// .solution/8.2-sv39-translate.rs — SV39 地址翻译参考实现
//
// 本文件是 feature 8.2 的完整参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 中实现 SV39 地址翻译。
//
// ## 教学概念：SV39 多级页表翻译
//
// SV39 使用三级页表将 39 位虚拟地址翻译为 56 位物理地址：
//
// 虚拟地址格式:
//   63    39 38    30 29    21 20    12 11       0
//   [保留]   [VPN[2]] [VPN[1]] [VPN[0]] [页内偏移]
//    25位      9位      9位      9位      12位
//
// 翻译流程:
//   satp.PPN → 根页表 (Level 2)
//     → VPN[2] 索引 → PTE → 下一级页表 (Level 1)
//     → VPN[1] 索引 → PTE → 下一级页表 (Level 0)
//     → VPN[0] 索引 → PTE → 物理页号 (PPN)
//   物理地址 = PPN × 4096 + 页内偏移
//
// 多级页表的优势：未使用的虚拟地址区域不需要分配页表页，节省内存。

// ============================================================================
// 常量
// ============================================================================

pub const PAGE_SIZE: usize = 4096;
pub const PAGE_OFFSET_BITS: usize = 12;
pub const VPN_BITS: usize = 9;
pub const PAGE_TABLE_LEVELS: usize = 3;

// PTE 标志位
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_X: u64 = 1 << 3;

const PPN_MASK: u64 = 0x0000_FFFF_FFFF_FC00;
const PPN_OFFSET: u64 = 10;

// ============================================================================
// PTE (简化版，仅用于翻译)
// ============================================================================

#[derive(Clone, Copy)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn from_bits(bits: u64) -> Self { Self(bits) }
    pub const fn is_valid(self) -> bool { self.0 & PTE_V != 0 }
    pub const fn ppn(self) -> usize {
        ((self.0 & PPN_MASK) >> PPN_OFFSET) as usize
    }
}

// ============================================================================
// 虚拟地址解析
// ============================================================================

/// 从虚拟地址提取 VPN（虚拟页号 = va >> 12）
pub const fn va_to_vpn(va: usize) -> usize {
    va >> PAGE_OFFSET_BITS
}

/// 从虚拟地址提取页内偏移（低 12 位）
pub const fn va_page_offset(va: usize) -> usize {
    va & (PAGE_SIZE - 1)
}

/// 从 VPN 提取指定级别的索引
///
/// - Level 2: VPN[2] = (vpn >> 18) & 0x1FF
/// - Level 1: VPN[1] = (vpn >> 9)  & 0x1FF
/// - Level 0: VPN[0] = (vpn >> 0)  & 0x1FF
pub const fn vpn_level_index(vpn: usize, level: usize) -> usize {
    (vpn >> (level * VPN_BITS)) & 0x1FF
}

// ============================================================================
// SV39 地址翻译
// ============================================================================

/// SV39 地址翻译错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TranslateError {
    /// 页表项无效（V=0）
    InvalidPte,
    /// 页表指针无效
    InvalidTable,
    /// 地址未映射
    NotMapped,
}

/// SV39 地址翻译：虚拟地址 → 物理地址
///
/// 遍历三级页表，将虚拟地址翻译为物理地址。
///
/// # 参数
/// - `va`: 要翻译的虚拟地址
/// - `root_ppn`: 根页表的物理页号（通常从 satp 寄存器获取）
/// - `phys_read`: 从物理地址读取 u64 的函数
///
/// # 返回
/// - `Ok(pa)`: 翻译后的物理地址
/// - `Err(e)`: 翻译失败的原因
pub fn translate_va(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
) -> Result<usize, TranslateError> {
    let vpn = va_to_vpn(va);
    let offset = va_page_offset(va);

    // 从根页表开始遍历
    let mut current_ppn = root_ppn;

    // 从 Level 2 遍历到 Level 0
    for level in (0..PAGE_TABLE_LEVELS).rev() {
        // 计算当前级别的 VPN 索引
        let idx = vpn_level_index(vpn, level);

        // 计算 PTE 的物理地址：页表基地址 + 索引 × 8
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        // 从物理内存读取 PTE
        let pte_val = phys_read(pte_addr).map_err(|_| TranslateError::InvalidTable)?;
        let pte = PageTableEntry::from_bits(pte_val);

        // 检查 PTE 是否有效
        if !pte.is_valid() {
            return Err(TranslateError::InvalidPte);
        }

        if level == 0 {
            // 到达最低级：叶子 PTE，包含最终的物理页号
            let pa = pte.ppn() * PAGE_SIZE + offset;
            return Ok(pa);
        }

        // 中间级别：PTE 指向下一级页表
        current_ppn = pte.ppn();
    }

    // 不应该到达这里
    Err(TranslateError::NotMapped)
}
