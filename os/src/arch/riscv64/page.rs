//! SV39 页表项 (Page Table Entry) 定义
//!
//! RISC-V SV39 使用 39 位虚拟地址，三级页表结构：
//! - 每个页表项 (PTE) 占 8 字节 (64 位)
//! - 每个页表页包含 512 个 PTE (4KB / 8B)
//! - 三级页表：VPN[2] → VPN[1] → VPN[0] → 物理页号
//!
//! ## 教学概念：SV39 虚拟地址格式
//!
//! ```text
//! 63    39 38    30 29    21 20    12 11       0
//! [保留]   [VPN[2]] [VPN[1]] [VPN[0]] [页内偏移]
//!  25位      9位      9位      9位      12位
//! ```
//!
//! ## SV39 PTE 格式 (64 位)
//!
//! ```text
//! 63    54 53    28 27    19 18    10 9  8 7 6 5 4 3 2 1 0
//! [保留]   [PPN[2]] [PPN[1]] [PPN[0]] [RSW] [D A G U X W R V]
//!  10位     26位      9位      9位     2位   各1位
//! ```
//!
//! - V (Valid): PTE 是否有效
//! - R (Read): 页面可读
//! - W (Write): 页面可写
//! - X (Execute): 页面可执行
//! - U (User): 用户态可访问
//! - G (Global): 全局映射（不随 ASID 刷新）
//! - A (Accessed): 已被访问（硬件或软件设置）
//! - D (Dirty): 已被写入（硬件或软件设置）

use core::fmt;

// ============================================================================
// SV39 PTE 标志位常量
// ============================================================================

/// SV39 页表项标志位
///
/// 每个标志位对应 PTE 的一个特定 bit。
/// 通过位运算组合多个标志。
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct PteFlags(pub u64);

impl PteFlags {
    /// V (Valid) — PTE 有效位
    /// 硬件检查：V=0 时访问该页会触发页错误
    pub const VALID: u64 = 1 << 0;

    /// R (Read) — 页面可读
    /// V=1 且 R=0 且 W=0 且 X=0: 这是一个指向下一级页表的 PTE（非叶子）
    pub const READ: u64 = 1 << 1;

    /// W (Write) — 页面可写
    pub const WRITE: u64 = 1 << 2;

    /// X (Execute) — 页面可执行
    pub const EXECUTE: u64 = 1 << 3;

    /// U (User) — 用户态可访问
    /// U=0: 仅 S-mode 可访问
    /// U=1: U-mode 和 S-mode 都可访问（受 SUM 位影响）
    pub const USER: u64 = 1 << 4;

    /// G (Global) — 全局映射
    /// 全局页在 SFENCE.VMA 时不随 ASID 刷新
    pub const GLOBAL: u64 = 1 << 5;

    /// A (Accessed) — 已访问
    /// 硬件在首次访问时设置（如果支持），否则软件必须设置
    pub const ACCESSED: u64 = 1 << 6;

    /// D (Dirty) — 已写入
    /// 硬件在首次写入时设置（如果支持），否则软件必须设置
    pub const DIRTY: u64 = 1 << 7;

    // --- 组合标志 ---

    /// 只读页: V + R
    pub const READ_ONLY: u64 = Self::VALID | Self::READ;

    /// 可读写页: V + R + W
    pub const READ_WRITE: u64 = Self::VALID | Self::READ | Self::WRITE;

    /// 可读可执行页: V + R + X
    pub const READ_EXECUTE: u64 = Self::VALID | Self::READ | Self::EXECUTE;

    /// 用户态只读: V + R + U
    pub const USER_READ_ONLY: u64 = Self::VALID | Self::READ | Self::USER;

    /// 用户态可读写: V + R + W + U
    pub const USER_READ_WRITE: u64 = Self::VALID | Self::READ | Self::WRITE | Self::USER;

    /// 用户态可读可执行: V + R + X + U
    pub const USER_READ_EXECUTE: u64 = Self::VALID | Self::READ | Self::EXECUTE | Self::USER;

    /// 创建新的标志
    pub const fn new(bits: u64) -> Self {
        Self(bits)
    }

    /// 获取原始位
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// 是否包含指定标志
    pub const fn contains(self, flag: u64) -> bool {
        (self.0 & flag) != 0
    }

    /// 是否是叶子 PTE（R 或 X 为 1）
    ///
    /// SV39 规范：V=1 且 (R|X) != 0 → 叶子 PTE（映射到物理页）
    ///           V=1 且 R=0 且 X=0  → 非叶子 PTE（指向下一级页表）
    pub const fn is_leaf(self) -> bool {
        self.0 & (Self::READ | Self::EXECUTE) != 0
    }
}

// ============================================================================
// SV39 页表项结构
// ============================================================================

/// SV39 常量：PTE 标志位掩码（低 8 位）
const SV39_FLAG_MASK: u64 = 0xFF;

/// SV39 常量：PPN 起始位
const SV39_PPN_OFFSET: u64 = 10;

/// SV39 常量：PPN 掩码（位 10-53）
const SV39_PPN_MASK: u64 = 0x0000_FFFF_FFFF_FC00;

/// SV39 页表项 (Page Table Entry)
///
/// 内部存储为 64 位整数，位布局遵循 RISC-V SV39 规范。
///
/// ## 教学概念：PTE 是硬件直接解释的数据结构
///
/// 与普通 Rust 结构体不同，PTE 的每一位都有硬件含义：
/// - CPU 的 MMU 会直接读取 PTE 来翻译地址
/// - 如果 PTE 格式不对，CPU 会触发页错误
/// - 因此 PTE 的位布局是**硬件规范**决定的，不是程序员自由选择的
///
/// 我们使用 `u64` 包装而不是 `#[repr(C)]` 结构体，
/// 因为标志位和 PPN 跨越了字节边界，用位操作更清晰。
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    /// 创建空 PTE（所有位为零，即无效 PTE）
    pub const fn empty() -> Self {
        Self(0)
    }

    /// 从原始 64 位值创建 PTE
    ///
    /// # Safety
    ///
    /// 调用者必须确保位值符合 SV39 PTE 格式。
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits)
    }

    /// 获取原始 64 位值
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// 创建叶子 PTE（映射到物理页）
    ///
    /// - `ppn`: 物理页号（44 位，页粒度 4KB）
    /// - `flags`: 标志位（必须包含 V 位）
    pub const fn new_leaf(ppn: u64, flags: PteFlags) -> Self {
        // PPN 左移 10 位，与标志位组合
        Self((ppn << SV39_PPN_OFFSET) | flags.bits())
    }

    /// 创建非叶子 PTE（指向下一级页表）
    ///
    /// 非叶子 PTE 只设置 V 位，R=W=X=0。
    pub const fn new_table(ppn: u64) -> Self {
        Self((ppn << SV39_PPN_OFFSET) | PteFlags::VALID)
    }

    /// PTE 是否有效（V 位是否设置）
    pub const fn is_valid(self) -> bool {
        self.0 & PteFlags::VALID != 0
    }

    /// PTE 是否为空（所有位为零）
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// 是否是叶子 PTE（映射到物理页，而非指向下一级页表）
    pub const fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & (PteFlags::READ | PteFlags::EXECUTE) != 0)
    }

    /// 获取物理页号 (PPN)
    ///
    /// PPN 占据 PTE 的位 10-53（共 44 位）。
    /// 返回的是页号，不是物理地址！物理地址 = PPN × 4096。
    pub const fn ppn(self) -> u64 {
        (self.0 & SV39_PPN_MASK) >> SV39_PPN_OFFSET
    }

    /// 获取标志位
    pub const fn flags(self) -> PteFlags {
        PteFlags(self.0 & SV39_FLAG_MASK)
    }

    /// 设置物理页号
    pub fn set_ppn(&mut self, ppn: u64) {
        self.0 = (self.0 & !SV39_PPN_MASK) | (ppn << SV39_PPN_OFFSET);
    }

    /// 设置标志位
    pub fn set_flags(&mut self, flags: PteFlags) {
        self.0 = (self.0 & !SV39_FLAG_MASK) | flags.bits();
    }

    /// 添加标志位
    pub fn add_flags(&mut self, flags: PteFlags) {
        self.0 |= flags.bits();
    }

    /// 移除标志位
    pub fn remove_flags(&mut self, flags: PteFlags) {
        self.0 &= !flags.bits();
    }

    /// 清空 PTE
    pub fn clear(&mut self) {
        self.0 = 0;
    }
}

// --- Display 实现，方便调试 ---

impl fmt::Debug for PageTableEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PTE")
            .field("raw", &format_args!("{:#018x}", self.0))
            .field("ppn", &format_args!("{:#x}", self.ppn()))
            .field("valid", &self.is_valid())
            .field("leaf", &self.is_leaf())
            .field("flags", &self.flags())
            .finish()
    }
}

impl fmt::Debug for PteFlags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PteFlags(")?;
        if self.contains(Self::VALID) {
            write!(f, "V")?;
        }
        if self.contains(Self::READ) {
            write!(f, "R")?;
        }
        if self.contains(Self::WRITE) {
            write!(f, "W")?;
        }
        if self.contains(Self::EXECUTE) {
            write!(f, "X")?;
        }
        if self.contains(Self::USER) {
            write!(f, "U")?;
        }
        if self.contains(Self::GLOBAL) {
            write!(f, "G")?;
        }
        if self.contains(Self::ACCESSED) {
            write!(f, "A")?;
        }
        if self.contains(Self::DIRTY) {
            write!(f, "D")?;
        }
        write!(f, ")")
    }
}

// ============================================================================
// 编译期验证
// ============================================================================

/// PTE 大小 = 8 字节 (64 位)
const _: () = assert!(core::mem::size_of::<PageTableEntry>() == 8);

/// PTE 对齐 = 8 字节
const _: () = assert!(core::mem::align_of::<PageTableEntry>() == 8);

// ============================================================================
// SV39 地址空间常量
// ============================================================================

/// SV39 虚拟地址位数
pub const SV39_VA_BITS: usize = 39;

/// SV39 物理地址位数
pub const SV39_PA_BITS: usize = 56;

/// 页大小 (4KB)
pub const PAGE_SIZE: usize = 4096;

/// 页内偏移位数 (12 位)
pub const PAGE_OFFSET_BITS: usize = 12;

/// VPN 每级位数 (9 位)
pub const VPN_BITS: usize = 9;

/// 页表级数 (3 级)
pub const PAGE_TABLE_LEVELS: usize = 3;

/// 每个页表页包含的 PTE 数量 (512)
pub const PTE_PER_PAGE: usize = PAGE_SIZE / 8;

// ============================================================================
// SV39 虚拟地址解析
// ============================================================================

/// 从虚拟地址提取 VPN（虚拟页号）
///
/// SV39 虚拟地址格式：
/// ```text
/// 63    39 38    30 29    21 20    12 11       0
/// [保留]   [VPN[2]] [VPN[1]] [VPN[0]] [页内偏移]
///  25位      9位      9位      9位      12位
/// ```
///
/// VPN = 虚拟地址 >> 12（去掉页内偏移）
pub const fn va_to_vpn(va: usize) -> usize {
    va >> PAGE_OFFSET_BITS
}

/// 从虚拟地址提取页内偏移
pub const fn va_page_offset(va: usize) -> usize {
    va & (PAGE_SIZE - 1)
}

/// 从 VPN 提取指定级别的索引
///
/// SV39 三级页表：
/// - Level 2: VPN[2] = VPN >> 18 & 0x1FF
/// - Level 1: VPN[1] = VPN >> 9 & 0x1FF
/// - Level 0: VPN[0] = VPN >> 0 & 0x1FF
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
    /// 页表指针无效（PPN 超出物理内存范围）
    InvalidTable,
    /// 地址未映射
    NotMapped,
}

/// SV39 地址翻译：虚拟地址 → 物理地址
///
/// 遍历三级页表，将虚拟地址翻译为物理地址。
///
/// ## 教学概念：SV39 多级页表翻译流程
///
/// SV39 使用三级页表将 39 位虚拟地址翻译为 56 位物理地址：
///
/// ```text
/// 虚拟地址:  [VPN[2]] [VPN[1]] [VPN[0]] [页内偏移]
///              9位      9位      9位      12位
///
/// satp.PPN → 根页表 (Level 2)
///               │
///               ├─ VPN[2] 索引 → PTE
///               │    │
///               │    ├─ PTE.PPN → 二级页表 (Level 1)
///               │    │              │
///               │    │              ├─ VPN[1] 索引 → PTE
///               │    │              │    │
///               │    │              │    ├─ PTE.PPN → 三级页表 (Level 0)
///               │    │              │    │              │
///               │    │              │    │              ├─ VPN[0] 索引 → PTE
///               │    │              │    │              │    │
///               │    │              │    │              │    └─ PTE.PPN → 物理页
///               │    │              │    │              │
///               └────┴──────────────┴────┴──────────────┘
///
/// 物理地址 = PTE.PPN × 4096 + 页内偏移
/// ```
///
/// 多级页表的优势：不需要映射整个地址空间，
/// 未使用的区域不需要分配页表页，节省内存。
///
/// # 参数
/// - `va`: 要翻译的虚拟地址
/// - `root_ppn`: 根页表的物理页号（通常从 satp 寄存器获取）
/// - `phys_read`: 从物理地址读取 u64 的函数（抽象物理内存访问）
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
            // 到达最低级：这是叶子 PTE，包含最终的物理页号
            let pa = pte.ppn() as usize * PAGE_SIZE + offset;
            return Ok(pa);
        }

        // 中间级别：PTE 指向下一级页表
        // 非叶子 PTE：V=1, R=W=X=0
        current_ppn = pte.ppn() as usize;
    }

    // 不应该到达这里（循环会处理所有级别）
    Err(TranslateError::NotMapped)
}

// ============================================================================
// SV39 页表解映射
// ============================================================================

/// SV39 页表解映射：清除虚拟页的映射
///
/// 遍历三级页表找到叶子 PTE，将其清零。
///
/// ## 教学概念：解映射与页表回收
///
/// 解映射一个虚拟页只需要将叶子 PTE 清零（V=0）。
/// 之后对该虚拟地址的访问会触发页错误。
///
/// 注意：简单实现不清除中间级别的页表页（即使它们变空了）。
/// 生产级 OS 会检查中间页表是否全空，如果是则回收该页。
///
/// # 参数
/// - `va`: 虚拟地址（必须页对齐）
/// - `root_ppn`: 根页表的物理页号
/// - `phys_read`: 从物理地址读取 u64 的函数
/// - `phys_write`: 写入 u64 到物理地址的函数
///
/// # 返回
/// - `Ok(())`: 解映射成功
/// - `Err(e)`: 解映射失败
pub fn unmap_page(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
    phys_write: fn(usize, u64) -> Result<(), ()>,
) -> Result<(), MapError> {
    debug_assert!(va % PAGE_SIZE == 0, "va must be page-aligned");

    let vpn = va_to_vpn(va);
    let mut current_ppn = root_ppn;

    // 从 Level 2 遍历到 Level 0
    for level in (0..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        let pte_val = phys_read(pte_addr).map_err(|_| MapError::PhysAccessFailed)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            return Err(MapError::NotMapped);
        }

        if level == 0 {
            // 叶子 PTE：清零以解除映射
            phys_write(pte_addr, 0).map_err(|_| MapError::PhysAccessFailed)?;
            return Ok(());
        }

        // 中间级别：继续下一级
        current_ppn = pte.ppn() as usize;
    }

    Err(MapError::NotMapped)
}

// ============================================================================
// SV39 页表映射
// ============================================================================

/// 页表映射错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MapError {
    /// 帧分配失败（无空闲物理页）
    FrameAllocFailed,
    /// 地址已被映射
    AlreadyMapped,
    /// 地址未映射
    NotMapped,
    /// 物理内存访问失败
    PhysAccessFailed,
}

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
///
/// # 返回
/// - `Ok(())`: 映射成功
/// - `Err(e)`: 映射失败的原因
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

// ============================================================================
// SV39 页表查询（lookup）
// ============================================================================

/// 页表查询错误
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LookupError {
    /// 地址未映射
    NotMapped,
    /// 物理内存访问失败
    PhysAccessFailed,
}

/// 查询虚拟地址对应的物理页号和标志位
///
/// 遍历三级页表，返回叶子 PTE 的物理页号和标志位。
///
/// ## 教学概念：地址查询
///
/// 与 `translate_va` 类似，但返回更多信息（PPN + flags）。
/// 用于检查映射是否存在、获取权限标志等。
///
/// # 参数
/// - `va`: 要查询的虚拟地址
/// - `root_ppn`: 根页表的物理页号
/// - `phys_read`: 从物理地址读取 u64 的函数
///
/// # 返回
/// - `Ok((ppn, flags))`: 物理页号和标志位
/// - `Err(e)`: 查询失败的原因
pub fn lookup_page(
    va: usize,
    root_ppn: usize,
    phys_read: fn(usize) -> Result<u64, ()>,
) -> Result<(usize, PteFlags), LookupError> {
    let vpn = va_to_vpn(va);
    let mut current_ppn = root_ppn;

    // 从 Level 2 遍历到 Level 0
    for level in (0..PAGE_TABLE_LEVELS).rev() {
        let idx = vpn_level_index(vpn, level);
        let pte_addr = current_ppn * PAGE_SIZE + idx * 8;

        let pte_val = phys_read(pte_addr).map_err(|_| LookupError::PhysAccessFailed)?;
        let pte = PageTableEntry::from_bits(pte_val);

        if !pte.is_valid() {
            return Err(LookupError::NotMapped);
        }

        if level == 0 {
            // 到达叶子 PTE：返回 PPN 和 flags
            return Ok((pte.ppn() as usize, pte.flags()));
        }

        // 中间级别：继续下一级
        current_ppn = pte.ppn() as usize;
    }

    Err(LookupError::NotMapped)
}
