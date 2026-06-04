//! # sys_sbrk 实现
//!
//! 参考实现：调整堆大小系统调用。
//!
//! ## 教学概念
//! sbrk (set break) 是 Unix 最经典的内存分配原语：
//! - 程序的堆从一个起始地址开始，通过 sbrk 向上扩展
//! - "break" 就是堆的顶部地址
//! - 正增量扩展堆，负增量收缩堆
//!
//! 在真实内核中，sys_sbrk 的工作流程：
//! 1. 获取当前任务的堆顶地址 (brk)
//! 2. 计算新堆顶 = brk + increment
//! 3. 如果 increment > 0：分配物理帧，映射到新的虚拟地址范围
//! 4. 如果 increment < 0：取消映射，释放物理帧
//! 5. 更新任务的堆顶记录
//! 6. 返回旧堆顶地址
//!
//! 在 Mock 环境下，没有全局 current_task 和真实页表，
//! 返回一个固定的 mock 堆顶地址。

/// 调整堆大小系统调用。
///
/// 扩展或缩小当前任务的堆空间。
///
/// # 参数
/// - `increment`: 堆增量（正数扩展，负数缩小，0 查询当前堆顶）
///
/// # 返回值
/// 成功返回旧堆顶地址，失败返回 -1。
pub fn sys_sbrk(increment: isize) -> usize {
    // 在真实内核中：
    // 1. 获取当前任务
    //    let current = current_task();
    //    let mut task = current.lock();
    // 2. 记录旧堆顶
    //    let old_brk = task.heap_top;
    // 3. 计算新堆顶
    //    let new_brk = (old_brk as isize + increment) as usize;
    // 4. 如果 increment > 0，分配新帧并映射
    //    if increment > 0 {
    //        for page in page_range(old_brk, new_brk) {
    //            let frame = frame_allocator.alloc()?;
    //            page_table.map(page, frame, PteFlags::READ | PteFlags::WRITE | PteFlags::USER)?;
    //        }
    //    }
    // 5. 如果 increment < 0，取消映射并释放帧
    //    if increment < 0 {
    //        for page in page_range(new_brk, old_brk) {
    //            let frame = page_table.unmap(page)?;
    //            frame_allocator.dealloc(frame);
    //        }
    //    }
    // 6. 更新堆顶
    //    task.heap_top = new_brk;
    //    old_brk
    //
    // Mock: 返回一个固定的堆顶地址
    let _ = increment; // 标记参数已使用
    0x8080_0000
}
