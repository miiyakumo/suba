// .solution/8.3-sv39-map.rs — SV39 页表映射参考实现
//
// 本文件是 feature 8.3 的完整参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 中的 TODO(student) 处自行实现。

// ============================================================================
// 页表映射错误
// ============================================================================

/// 页表映射错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// 帧分配失败（无空闲物理页）
    FrameAllocFailed,
    /// 地址已被映射
    AlreadyMapped,
    /// 物理内存访问失败
    PhysAccessFailed,
}

// ============================================================================
// SV39 页表映射
// ============================================================================

/// SV39 页表映射：建立虚拟页 → 物理页的映射
///
/// 在三级页表中逐级查找/创建页表项，最终在 Level 0 创建叶子 PTE。
///
/// ## 教学概念：页表映射流程
///
/// 映射一个虚拟页到物理页需要：
/// 1. 从根页表开始，按 VPN[2] → VPN[1] → VPN[0] 逐级查找
/// 2. 如果中间级别的 PTE 无效（V=0），分配新的页表页
/// 3. 在 Level 0 创建叶子 PTE（V=1, R/W/X 根据权限设置）
///
/// ```text
/// map_page(va=0x1000_0000, pa=0x8020_0000, flags=VRWX)
///
/// 根页表 (Level 2):
///   [VPN[2]=64] → PTE (V=1, PPN=新分配的二级页表)
///
/// 二级页表 (Level 1):
///   [VPN[1]=0]  → PTE (V=1, PPN=新分配的三级页表)
///
/// 三级页表 (Level 0):
///   [VPN[0]=0]  → PTE (V=1, R=1, W=1, X=1, PPN=0x80200)
///                  物理地址 = 0x80200 × 4096 + 偏移 = 0x8020_0000
/// ```
///
/// # 参数
/// - `va`: 虚拟地址（必须页对齐）
/// - `pa`: 物理地址（必须页对齐）
/// - `flags`: PTE 标志位（PteFlags）
/// - `root_ppn`: 根页表的物理页号
/// - `phys_read`: 从物理地址读取 u64 的函数
/// - `phys_write`: 写入 u64 到物理地址的函数
/// - `alloc_frame`: 分配一个物理帧的函数，返回新帧的 PPN
pub fn map_page(
    va: usize,
    pa: usize,
    flags: PteFlags,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
    phys_write: fn(usize, u64) -> Result<(), ()>,
    alloc_frame: fn() -> Option<usize>,
) -> Result<(), MapError> {
    // 页对齐检查
    debug_assert!(va % PAGE_SIZE == 0, "va must be page-aligned");
    debug_assert!(pa % PAGE_SIZE == 0, "pa must be page-aligned");

    let vpn = va_to_vpn(va);
    let target_ppn = pa / PAGE_SIZE;

    // 从根页表开始遍历
    let mut current_ppn = root_ppn;

    // 从 Level 2 遍历到 Level 1（中间级别）
    for level in (1..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        // 读取当前 PTE
        let pte_val = phys_read(pte_addr).map_err(|_| MapError::PhysAccessFailed)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            // PTE 无效：需要分配新的页表页
            let new_ppn = alloc_frame().ok_or(MapError::FrameAllocFailed)?;

            // 清零新页表页（确保所有 PTE 无效）
            for i in 0..PTE_PER_PAGE {
                let clear_addr = new_ppn * PAGE_SIZE + i * 8;
                phys_write(clear_addr, 0).map_err(|_| MapError::PhysAccessFailed)?;
            }

            // 创建非叶子 PTE：指向新页表页
            let new_pte = PageTableEntry::new_table(new_ppn as u64);
            phys_write(pte_addr, new_pte.bits()).map_err(|_| MapError::PhysAccessFailed)?;

            current_ppn = new_ppn;
        } else {
            // PTE 有效：继续下一级
            current_ppn = pte.ppn() as usize;
        }
    }

    // Level 0：创建叶子 PTE
    let idx = vpn_level_index(vpn, 0);
    let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

    // 检查是否已映射
    let pte_val = phys_read(pte_addr).map_err(|_| MapError::PhysAccessFailed)?;
    let pte = PageTableEntry::from_bits(pte_val);
    if pte.is_valid() {
        return Err(MapError::AlreadyMapped);
    }

    // 创建叶子 PTE：V=1 + 用户指定的标志位 + PPN
    let leaf_pte = PageTableEntry::new_leaf(target_ppn as u64, flags);
    phys_write(pte_addr, leaf_pte.bits()).map_err(|_| MapError::PhysAccessFailed)?;

    Ok(())
}
