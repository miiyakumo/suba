// .solution/8.1-sv39-pte.rs — SV39 页表项定义参考实现
//
// 本文件是 feature 8.1 的完整参考实现。
// 学生应在 os/src/arch/riscv64/page.rs 中定义 SV39 PTE 结构。
//
// ## 教学概念：SV39 PTE 格式
//
// SV39 使用 39 位虚拟地址，三级页表：
// - 每个 PTE = 8 字节 (64 位)
// - 每个页表页 = 512 个 PTE (4KB)
// - 三级索引：VPN[2] → VPN[1] → VPN[0] → PPN
//
// PTE 位布局:
//   bit 0:     V (Valid)
//   bit 1:     R (Read)
//   bit 2:     W (Write)
//   bit 3:     X (Execute)
//   bit 4:     U (User)
//   bit 5:     G (Global)
//   bit 6:     A (Accessed)
//   bit 7:     D (Dirty)
//   bit 8-9:   RSW (保留给软件)
//   bit 10-53: PPN (物理页号)
//   bit 54-63: 保留

// 标志位常量
pub const PTE_V: u64 = 1 << 0;
pub const PTE_R: u64 = 1 << 1;
pub const PTE_W: u64 = 1 << 2;
pub const PTE_X: u64 = 1 << 3;
pub const PTE_U: u64 = 1 << 4;
pub const PTE_G: u64 = 1 << 5;
pub const PTE_A: u64 = 1 << 6;
pub const PTE_D: u64 = 1 << 7;

// PTE 结构体
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    pub const fn empty() -> Self { Self(0) }
    pub const fn from_bits(bits: u64) -> Self { Self(bits) }
    pub const fn bits(self) -> u64 { self.0 }

    pub const fn new_leaf(ppn: u64, flags: u64) -> Self {
        Self((ppn << 10) | flags)
    }
    pub const fn new_table(ppn: u64) -> Self {
        Self((ppn << 10) | PTE_V)
    }

    pub const fn is_valid(self) -> bool { self.0 & PTE_V != 0 }
    pub const fn is_leaf(self) -> bool {
        self.is_valid() && (self.0 & (PTE_R | PTE_X)) != 0
    }
    pub const fn ppn(self) -> u64 {
        (self.0 & 0x0000_FFFF_FFFF_FC00) >> 10
    }
    pub const fn flags(self) -> u64 { self.0 & 0xFF }
}
