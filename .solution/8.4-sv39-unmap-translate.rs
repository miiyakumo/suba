// .solution/8.4-sv39-unmap-translate.rs — 页表 unmap 和 translate 参考实现
//
// 本文件是 feature 8.4 的完整参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 中实现 unmap_page 和 lookup_page。

// ============================================================================
// translate_va — 虚拟地址翻译
// ============================================================================

/// SV39 地址翻译：虚拟地址 → 物理地址
///
/// 遍历三级页表，将虚拟地址翻译为物理地址。
///
/// 流程：
/// 1. 从 root_ppn 指向的根页表开始
/// 2. 用 VPN[2] 索引 Level 2 页表，获取 PTE
/// 3. 如果 PTE 无效 → 返回错误
/// 4. 如果 PTE 是叶子 → 计算物理地址
/// 5. 如果 PTE 是非叶子 → 用 PPN 找到下一级页表，继续
/// 6. 到达 Level 0 时，PPN + offset = 物理地址
pub fn translate_va(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
) -> Result<usize, TranslateError> {
    let vpn = va_to_vpn(va);
    let offset = va_page_offset(va);
    let mut current_ppn = root_ppn;

    for level in (0..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        let pte_val = phys_read(pte_addr).map_err(|_| TranslateError::InvalidTable)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            return Err(TranslateError::InvalidPte);
        }

        if level == 0 {
            let pa = pte.ppn() as usize * PAGE_SIZE + offset;
            return Ok(pa);
        }

        current_ppn = pte.ppn() as usize;
    }

    Err(TranslateError::NotMapped)
}

// ============================================================================
// unmap_page — 清除页表映射
// ============================================================================

/// 解除虚拟页的映射
///
/// 遍历三级页表，将叶子 PTE 清零。
/// 注意：简单实现不回收空的中间页表页。
pub fn unmap_page(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
    phys_write: fn(usize, u64) -> Result<(), ()>,
) -> Result<(), MapError> {
    let vpn = va_to_vpn(va);
    let mut current_ppn = root_ppn;

    for level in (0..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        let pte_val = phys_read(pte_addr).map_err(|_| MapError::PhysAccessFailed)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            return Err(MapError::NotMapped);
        }

        if level == 0 {
            phys_write(pte_addr, 0).map_err(|_| MapError::PhysAccessFailed)?;
            return Ok(());
        }

        current_ppn = pte.ppn() as usize;
    }

    Err(MapError::NotMapped)
}

// ============================================================================
// lookup_page — 查询页表映射
// ============================================================================

/// 查询虚拟地址对应的物理页号和标志位
///
/// 遍历三级页表，返回叶子 PTE 的物理页号和标志位。
/// 用于检查映射是否存在、获取权限标志等。
pub fn lookup_page(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
) -> Result<(usize, PteFlags), LookupError> {
    let vpn = va_to_vpn(va);
    let mut current_ppn = root_ppn;

    for level in (0..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        let pte_val = phys_read(pte_addr).map_err(|_| LookupError::PhysAccessFailed)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            return Err(LookupError::NotMapped);
        }

        if level == 0 {
            return Ok((pte.ppn() as usize, pte.flags()));
        }

        current_ppn = pte.ppn() as usize;
    }

    Err(LookupError::NotMapped)
}
