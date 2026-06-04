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
