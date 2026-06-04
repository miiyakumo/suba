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

// ============================================================================
// 物理帧分配器（Bump 风格）
// ============================================================================

/// 全局物理帧分配器
///
/// 使用简单的 bump 策略分配物理帧。每个帧 4KB。
/// 在内核启动时由 `init_frame_allocator()` 初始化。
///
/// ## 教学概念：物理帧分配
///
/// 内核需要动态分配物理页来创建用户页表、映射用户内存等。
/// 这里使用最简单的 bump 分配器——分配指针只增不减。
/// 生产级 OS 会使用 buddy allocator 或 slab allocator。
static mut FRAME_ALLOC_NEXT: usize = 0;
static mut FRAME_ALLOC_END: usize = 0;

/// 初始化物理帧分配器
///
/// # 参数
/// - `start_pa`: 可分配物理内存的起始地址（必须页对齐）
/// - `end_pa`: 可分配物理内存的结束地址
///
/// # Safety
///
/// 必须在单线程启动阶段调用一次。
/// `[start_pa, end_pa)` 范围内的物理内存必须可用且不与内核冲突。
pub unsafe fn init_frame_allocator(start_pa: usize, end_pa: usize) {
    // SAFETY: 单线程启动阶段写入全局状态
    unsafe {
        FRAME_ALLOC_NEXT = (start_pa + PAGE_SIZE - 1) & !(PAGE_SIZE - 1); // 页对齐
        FRAME_ALLOC_END = end_pa & !(PAGE_SIZE - 1);
    }
}

/// 分配一个物理帧，返回物理页号 (PPN)
///
/// 使用 bump 策略：每次分配推进指针一个页。
/// 返回 `None` 表示物理内存耗尽。
pub fn alloc_frame() -> Option<usize> {
    // SAFETY: 单核环境下无竞争（后续可改为 spin::Mutex 保护）
    unsafe {
        let next = FRAME_ALLOC_NEXT;
        if next + PAGE_SIZE > FRAME_ALLOC_END {
            return None;
        }
        FRAME_ALLOC_NEXT = next + PAGE_SIZE;
        Some(next / PAGE_SIZE)
    }
}

/// 分配一个物理帧并清零
///
/// 返回清零后的物理页号。页表分配必须清零——
/// 未初始化的 PTE 可能被硬件误认为有效。
pub fn alloc_zeroed_frame() -> Option<usize> {
    let ppn = alloc_frame()?;
    let pa = ppn * PAGE_SIZE;
    // SAFETY: 刚分配的物理帧，我们拥有独占访问权
    unsafe {
        core::ptr::write_bytes(pa as *mut u8, 0, PAGE_SIZE);
    }
    Some(ppn)
}

/// 从物理地址读取 u64（用于页表遍历）
///
/// # Safety
///
/// `pa` 必须是有效的物理地址且 8 字节对齐。
fn phys_read_u64(pa: usize) -> Result<u64, ()> {
    // 在身份映射下，物理地址 = 虚拟地址
    // SAFETY: 调用者确保地址有效
    unsafe { Ok(core::ptr::read(pa as *const u64)) }
}

/// 写入 u64 到物理地址（用于页表修改）
///
/// # Safety
///
/// `pa` 必须是有效的物理地址且 8 字节对齐。
fn phys_write_u64(pa: usize, val: u64) -> Result<(), ()> {
    // 在身份映射下，物理地址 = 虚拟地址
    // SAFETY: 调用者确保地址有效
    unsafe {
        core::ptr::write(pa as *mut u64, val);
    }
    Ok(())
}

// ============================================================================
// 用户地址空间
// ============================================================================

/// Trampoline 页的虚拟地址
///
/// Trampoline 页映射在用户地址空间的最顶部（USER_TOP - PAGE_SIZE）。
/// 它包含 trap entry/return 代码，在用户态和内核态之间共享同一虚拟地址。
///
/// ## 教学概念：Trampoline 页
///
/// 当 CPU 从 U-mode 陷入 S-mode 时：
/// 1. 硬件跳转到 stvec（trap_entry 地址）
/// 2. 此时 satp 仍指向用户页表
/// 3. trap_entry 代码必须在用户页表中也能访问
///
/// 解决方案：将 trap entry 代码映射到用户地址空间顶部的 trampoline 页。
/// 这样即使 satp 还是用户页表，trap_entry 也能正常执行。
/// 进入内核后切换到内核页表，再跳转到内核地址的 trap 处理代码。
pub const TRAMPOLINE_VA: usize = suba_kernel::mm::address::USER_TOP - PAGE_SIZE;

/// 用户栈顶地址（trampoline 页下方）
///
/// 用户栈从 TRAMPOLINE_VA - PAGE_SIZE 向下增长。
pub const USER_STACK_TOP: usize = TRAMPOLINE_VA - PAGE_SIZE;

/// 默认用户栈大小（8MB = 2048 个 4KB 页）
pub const USER_STACK_SIZE: usize = 2048 * PAGE_SIZE;

/// 用户地址空间
///
/// 管理一个独立的 SV39 页表，包含：
/// - 内核空间映射（从当前内核页表复制）
/// - 用户代码/数据段映射（U 标志）
/// - 用户栈映射
/// - Trampoline 页映射
///
/// ## 教学概念：用户地址空间 vs 内核地址空间
///
/// 每个用户进程有自己的页表（不同的 satp 值）。
/// 用户页表包含两部分：
/// 1. **低地址（用户空间）**：进程私有的代码、数据、栈
/// 2. **高地址（内核空间）**：所有进程共享的内核映射
///
/// 这样设计的好处：
/// - 用户态只能访问自己的内存（隔离）
/// - 陷入内核时无需切换页表（内核映射已在高位）
/// - 内核可以通过页表高位访问自己的代码和数据
pub struct UserAddrSpace {
    /// 根页表的物理页号
    root_ppn: usize,
}

impl UserAddrSpace {
    /// 创建新的用户地址空间
    ///
    /// 1. 分配一个物理帧作为根页表
    /// 2. 从当前内核页表复制高地址（内核空间）映射
    /// 3. 低地址（用户空间）初始为空
    ///
    /// ## 教学概念：页表复制
    ///
    /// SV39 的根页表有 512 个 PTE，每个覆盖 1GB：
    /// - PTE[0..256]: 用户空间（0x0 ~ 0x0000_003F_FFFF_FFFF）
    /// - PTE[256..512]: 内核空间（0xFFFF_FC00_0000_0000 ~ ）
    ///
    /// 创建用户页表时，复制内核部分的 PTE（高 256 个），
    /// 用户部分的 PTE 全部清零（后续按需映射）。
    ///
    /// # 返回
    /// - `Ok(space)`: 新的用户地址空间
    /// - `Err(MapError)`: 帧分配失败
    pub fn new() -> Result<Self, MapError> {
        // 分配并清零根页表帧
        let root_ppn = alloc_zeroed_frame().ok_or(MapError::FrameAllocFailed)?;

        // 从当前内核页表复制所有内核映射
        // 读取当前 satp 获取内核根页表 PPN
        let kernel_satp = read_satp_current();
        let kernel_root_ppn = kernel_satp & 0x0FFF_FFFF_FFFF; // 低 44 位是 PPN

        // 复制内核 PTE 到用户页表
        //
        // ## 教学概念：内核映射复制
        //
        // 我们的内核使用身份映射（VA = PA），通过 1GB 大页映射了 4GB 物理地址空间。
        // 当用户态 trap 进入内核时，satp 仍指向用户页表，
        // 内核代码必须在用户页表中也可访问。
        //
        // 复制全部 512 个 PTE 确保内核的完整映射在用户页表中可用。
        //
        // **约束**：用户代码的虚拟地址不能与内核的 1GB 大页映射冲突。
        // 内核映射了 0x0000_0000 ~ 0xFFFF_FFFF（4GB），所以用户代码应加载到
        // 0x1_0000_0000（4GB）以上的虚拟地址，或使用不同的映射策略。
        for i in 0..512 {
            let src_addr = kernel_root_ppn * PAGE_SIZE + i * 8;
            let dst_addr = root_ppn * PAGE_SIZE + i * 8;
            if let Ok(pte_val) = phys_read_u64(src_addr) {
                let _ = phys_write_u64(dst_addr, pte_val);
            }
        }

        Ok(Self { root_ppn })
    }

    /// 获取根页表的物理页号
    ///
    /// 用于设置 satp 寄存器：`satp = (8 << 60) | root_ppn`
    pub fn root_ppn(&self) -> usize {
        self.root_ppn
    }

    /// 映射一个用户页
    ///
    /// 在用户页表中建立虚拟页 → 物理页的映射。
    /// 自动添加 U (User) 标志，允许用户态访问。
    ///
    /// # 参数
    /// - `va`: 虚拟地址（必须页对齐）
    /// - `pa`: 物理地址（必须页对齐）
    /// - `flags`: 额外的 PTE 标志（U 标志会自动添加）
    ///
    /// # Safety
    ///
    /// 调用者必须确保物理地址有效且不与其他映射冲突。
    pub unsafe fn map_user_page(
        &self,
        va: usize,
        pa: usize,
        flags: PteFlags,
    ) -> Result<(), MapError> {
        // 自动添加 U 标志
        let user_flags = PteFlags(flags.0 | PteFlags::USER);
        map_page(
            va,
            pa,
            user_flags,
            self.root_ppn,
            phys_read_u64,
            phys_write_u64,
            alloc_zeroed_frame,
        )
    }

    /// 映射一个内核页（不带 U 标志）
    ///
    /// 用于映射 trampoline 页等内核可执行但用户也可访问的页面。
    ///
    /// # Safety
    ///
    /// 调用者必须确保物理地址有效。
    pub unsafe fn map_kernel_page(
        &self,
        va: usize,
        pa: usize,
        flags: PteFlags,
    ) -> Result<(), MapError> {
        map_page(
            va,
            pa,
            flags,
            self.root_ppn,
            phys_read_u64,
            phys_write_u64,
            alloc_zeroed_frame,
        )
    }

    /// 映射用户代码/数据段
    ///
    /// 将 ELF 加载后的用户程序映射到用户地址空间。
    /// 根据 flags 设置正确的权限（R/W/X + U）。
    ///
    /// # 参数
    /// - `va`: 段的虚拟起始地址（页对齐）
    /// - `pa`: 段的物理起始地址（页对齐）
    /// - `size`: 段的大小（字节，会向上取整到页）
    /// - `flags`: PTE 标志（U 标志会自动添加）
    ///
    /// # Safety
    ///
    /// 调用者必须确保物理地址范围有效。
    pub unsafe fn map_segment(
        &self,
        va: usize,
        pa: usize,
        size: usize,
        flags: PteFlags,
    ) -> Result<(), MapError> {
        let num_pages = size.div_ceil(PAGE_SIZE);
        for i in 0..num_pages {
            let page_va = va + i * PAGE_SIZE;
            let page_pa = pa + i * PAGE_SIZE;
            // SAFETY: 调用者确保物理地址范围有效
            unsafe {
                self.map_user_page(page_va, page_pa, flags)?;
            }
        }
        Ok(())
    }

    /// 映射用户栈
    ///
    /// 在用户地址空间的高地址区域分配并映射用户栈。
    /// 栈从 `USER_STACK_TOP` 向下增长，大小为 `USER_STACK_SIZE`。
    ///
    /// ## 教学概念：用户栈布局
    ///
    /// ```text
    /// TRAMPOLINE_VA ───────────  ← USER_TOP - PAGE_SIZE
    ///   [trampoline 页]
    /// USER_STACK_TOP ───────────  ← TRAMPOLINE_VA - PAGE_SIZE
    ///   [用户栈 ↓ 向下增长]
    ///   ...
    ///   [栈底]
    /// ```
    ///
    /// # 返回
    /// - `Ok(stack_top_va)`: 用户栈顶虚拟地址
    /// - `Err(MapError)`: 帧分配失败
    pub fn map_user_stack(&self) -> Result<usize, MapError> {
        let num_pages = USER_STACK_SIZE / PAGE_SIZE;
        let stack_bottom_va = USER_STACK_TOP - USER_STACK_SIZE;

        for i in 0..num_pages {
            let va = stack_bottom_va + i * PAGE_SIZE;
            // 分配物理帧用于栈页
            let pa_ppn = alloc_zeroed_frame().ok_or(MapError::FrameAllocFailed)?;
            let pa = pa_ppn * PAGE_SIZE;
            // 栈页：可读写 + 用户态
            let flags = PteFlags(PteFlags::READ | PteFlags::WRITE);
            // SAFETY: 刚分配的物理帧，独占访问
            unsafe {
                self.map_user_page(va, pa, flags)?;
            }
        }

        Ok(USER_STACK_TOP)
    }

    /// 映射 trampoline 页
    ///
    /// 将 trap entry/return 代码映射到用户地址空间顶部。
    /// Trampoline 页在用户页表和内核页表中映射到相同的虚拟地址。
    ///
    /// ## 教学概念：为什么需要 Trampoline
    ///
    /// 当用户态发生 trap 时：
    /// 1. CPU 跳转到 stvec（trap_entry 的地址）
    /// 2. 此时 satp 仍然指向用户页表
    /// 3. trap_entry 代码必须在用户页表中可访问
    ///
    /// 解决方案：将 trap entry 代码映射到一个固定虚拟地址（trampoline），
    /// 在用户页表和内核页表中都映射到同一物理页。
    ///
    /// ```text
    /// 用户页表:  TRAMPOLINE_VA → trap_entry 物理页
    /// 内核页表:  TRAMPOLINE_VA → trap_entry 物理页 (同一物理页)
    /// ```
    ///
    /// # 参数
    /// - `trap_entry_pa`: trap entry 代码的物理地址
    ///
    /// # Safety
    ///
    /// `trap_entry_pa` 必须指向包含有效 trap entry 代码的物理页。
    pub unsafe fn map_trampoline(&self, trap_entry_pa: usize) -> Result<(), MapError> {
        // Trampoline 页：可读可执行（用户态 + 内核态都可访问）
        let flags = PteFlags(PteFlags::VALID | PteFlags::READ | PteFlags::EXECUTE);
        // SAFETY: 调用者确保 trap_entry_pa 有效
        unsafe {
            self.map_kernel_page(TRAMPOLINE_VA, trap_entry_pa, flags)?;
        }
        Ok(())
    }

    /// 激活此用户页表
    ///
    /// 将 satp 切换到此用户地址空间的根页表。
    ///
    /// # Safety
    ///
    /// 调用者必须确保页表已正确初始化，且当前不在用户态。
    pub unsafe fn activate(&self) {
        // SAFETY: root_ppn 指向有效的 SV39 页表
        unsafe {
            super::switch_page_table(self.root_ppn);
        }
    }
}

/// 读取当前 satp 寄存器值（内部使用）
fn read_satp_current() -> usize {
    let satp: usize;
    // SAFETY: satp 是只读 CSR
    unsafe {
        core::arch::asm!("csrr {}, satp", out(reg) satp, options(nomem, nostack));
    }
    satp
}
