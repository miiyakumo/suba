/// Mock 页表实现。
///
/// 使用简单的 Vec 存储映射关系，在测试环境中模拟页表操作。
pub struct MockPageTable {
    /// 映射列表：(虚拟地址, 物理地址, 标志)
    mappings: alloc::vec::Vec<(usize, usize, crate::arch::PteFlags)>,
}

impl MockPageTable {
    /// 创建新的 Mock 页表
    pub fn new() -> Self {
        Self {
            mappings: alloc::vec::Vec::new(),
        }
    }
}

impl Default for MockPageTable {
    fn default() -> Self {
        Self::new()
    }
}

impl PageTable for MockPageTable {
    unsafe fn map(&mut self, va: usize, pa: usize, flags: crate::arch::PteFlags) -> Result<(), ()> {
        // 检查是否已存在映射
        if self.mappings.iter().any(|(v, _, _)| *v == va) {
            return Err(());
        }
        self.mappings.push((va, pa, flags));
        Ok(())
    }

    fn unmap(&mut self, va: usize) -> Result<(), ()> {
        let len_before = self.mappings.len();
        self.mappings.retain(|(v, _, _)| *v != va);
        if self.mappings.len() < len_before {
            Ok(())
        } else {
            Err(())
        }
    }

    fn translate(&self, va: usize) -> Option<usize> {
        self.mappings
            .iter()
            .find(|(v, _, _)| *v == va)
            .map(|(_, pa, _)| *pa)
    }
}
