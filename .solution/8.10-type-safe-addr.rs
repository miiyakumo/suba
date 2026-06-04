// .solution/8.10-type-safe-addr.rs — 地址类型安全参考实现
//
// 本文件是 feature 8.10 的参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 中实现类型安全的地址操作。

// ============================================================================
// 类型安全的地址操作
// ============================================================================

/// 使用 PA/VA 类型包装器确保地址不会被误用。
///
/// ## 教学概念：类型安全的地址
///
/// 物理地址和虚拟地址虽然都是 `usize`，但语义完全不同：
/// - 物理地址 (PA): 硬件内存总线上的地址
/// - 虚拟地址 (VA): 程序看到的地址，需要页表翻译
///
/// 如果不小心把 PA 当 VA 使用（或反过来），会导致难以调试的 bug。
/// 通过 `#[repr(transparent)]` 包装器，编译器可以在编译期捕获这类错误。
///
/// 包装器是零成本的——`#[repr(transparent)]` 确保运行时与裸 `usize` 完全相同。

use suba_kernel::arch::{PA, VA};

/// 类型安全的地址翻译
///
/// 将虚拟地址 (VA) 翻译为物理地址 (PA)。
pub unsafe fn translate_va_typed(va: VA, root_ppn: usize) -> Result<PA, TranslateError> {
    let pa = translate_va(va.0, root_ppn, phys_read_u64)?;
    Ok(PA(pa))
}

/// 类型安全的页表映射
///
/// 建立虚拟地址 (VA) 到物理地址 (PA) 的映射。
pub fn map_page_typed(
    va: VA, pa: PA, flags: PteFlags, root_ppn: usize,
) -> Result<(), MapError> {
    map_page(va.0, pa.0, flags, root_ppn, phys_read_u64, phys_write_u64, alloc_zeroed_frame)
}

/// 类型安全的页表解映射
pub fn unmap_page_typed(va: VA, root_ppn: usize) -> Result<(), MapError> {
    unmap_page(va.0, root_ppn, phys_read_u64, phys_write_u64)
}

/// 类型安全的页表查询
pub fn lookup_page_typed(va: VA, root_ppn: usize) -> Result<(usize, PteFlags), LookupError> {
    lookup_page(va.0, root_ppn, phys_read_u64)
}

/// 从 VA 创建 VPN（虚拟页号）
pub fn va_to_vpn_typed(va: VA) -> usize {
    va_to_vpn(va.0)
}

/// 从 PA 创建 PPN（物理页号）
pub fn pa_to_ppn(pa: PA) -> usize {
    pa.0 / PAGE_SIZE
}

/// 从 PPN 创建 PA
pub fn ppn_to_pa(ppn: usize) -> PA {
    PA(ppn * PAGE_SIZE)
}

// === 教学要点 ===
//
// 1. #[repr(transparent)] 包装器是零成本的
//    - 运行时与裸 usize 完全相同
//    - 编译器在编译期检查类型
//
// 2. PA 和 VA 不能互相赋值
//    - let pa: PA = PA(0x80200000);
//    - let va: VA = VA(0x80200000);
//    - pa = va; // 编译错误！类型不匹配
//
// 3. 内部实现仍使用 usize
//    - 页表操作需要大量地址计算
//    - 使用 usize 避免频繁类型转换
//    - 公共 API 使用 PA/VA 确保安全
//
// 4. 这是 "newtype pattern" 的典型应用
//    - 用类型系统编码领域语义
//    - 编译器帮你检查正确性
//    - 零运行时开销
