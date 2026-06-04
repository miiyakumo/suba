//! # Task — 任务控制块
//!
//! 定义任务的数据结构和状态机。
//!
//! ## 教学概念
//! - **Task Control Block (TCB)**：操作系统中每个"执行流"的身份证
//! - **TaskState 状态机**：任务在其生命周期中经历的状态转换
//! - **PID**：进程标识符，内核中的唯一 ID

use crate::arch::Context;
use crate::fs::FdTable;

/// 任务状态枚举
///
/// ## 状态机转换
/// ```text
///     ┌──────┐
///     │ Ready │ ←── 新任务创建
///     └──┬───┘
///        │ 被调度器选中
///        ▼
///     ┌──────┐
///     │Running│
///     └──┬───┘
///        │
///   ┌────┼────┐
///   │    │    │
///   ▼    │    ▼
/// ┌───┐  │  ┌─────┐
/// │Wait│  │  │Exited│
/// └─┬─┘  │  └─────┘
///   │    │
///   └────┘→ 回到 Ready
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    /// 就绪态：等待被调度
    Ready,
    /// 运行态：正在 CPU 上执行
    Running,
    /// 等待态：等待某个事件（如 I/O 完成）
    Waiting,
    /// 退出态：任务已结束
    Exited,
}

/// 任务控制块 (TCB)。
///
/// 包含一个任务的所有内核管理信息。
/// 这是操作系统中最核心的数据结构之一。
///
/// ## 文件描述符
/// 每个任务创建时预分配三个标准文件描述符：
/// - fd 0: stdin（标准输入）
/// - fd 1: stdout（标准输出）
/// - fd 2: stderr（标准错误）
///
/// 这体现了 Unix "一切皆文件" 的理念——连终端输入输出
/// 也通过文件描述符访问。
///
/// ## 地址空间
/// 每个用户任务有独立的 SV39 页表（satp 字段）。
/// 内核任务的 satp = 0（使用内核页表，不切换）。
/// 上下文切换时，调度器激活目标任务的页表。
pub struct Task {
    /// 任务 ID
    pub pid: usize,
    /// 任务状态
    pub state: TaskState,
    /// 任务上下文（用于上下文切换）
    pub context: Context,
    /// 内核栈指针
    pub kstack_top: usize,
    /// 用户栈指针
    pub ustack_top: usize,
    /// 退出码
    pub exit_code: i32,
    /// 文件描述符表
    ///
    /// 创建时预分配 stdin(0)、stdout(1)、stderr(2)。
    pub fd_table: FdTable,
    /// 页表根物理页号（root_ppn）
    ///
    /// SV39 页表的根页表物理页号。
    /// - 内核任务：0（使用内核页表，不切换）
    /// - 用户任务：`UserAddrSpace::root_ppn()`
    ///
    /// 激活页表时：`satp = (8 << 60) | page_table_root`
    ///
    /// ## 教学概念：为什么每任务需要独立页表？
    ///
    /// 用户任务需要独立的虚拟地址空间来实现隔离：
    /// - 任务 A 不能读写任务 B 的内存
    /// - 每个任务看到的地址空间布局相同（代码/数据/栈）
    /// - 内核映射在高地址区域共享
    pub page_table_root: usize,

    /// 用户程序入口地址
    ///
    /// 仅用户任务使用。内核任务为 0。
    /// 用于 trampoline 函数：激活页表后 sret 到此地址。
    pub user_entry: usize,

    /// 陷阱帧指针（指向架构特定的 TrapFrame）
    ///
    /// ## 教学概念：为什么需要 trap_frame_ptr？
    ///
    /// 当任务被中断（如时钟中断）打断时，CPU 的全部寄存器被保存到 TrapFrame。
    /// 调度器切换任务时，需要知道每个任务的 TrapFrame 在哪里，
    /// 以便在任务被重新调度时恢复其寄存器状态。
    ///
    /// - 内核任务（如 idle）：trap_frame_ptr = 0（不需要，使用 context_switch 即可）
    /// - 用户任务：指向该任务专属的 TrapFrame（由 trap_entry 保存）
    ///
    /// ## 教学概念：forkret 路径
    ///
    /// 用户任务首次被调度时，context.ra 指向 forkret 函数。
    /// forkret 读取 trap_frame_ptr，将其写入 sscratch CSR，
    /// 然后调用 trap_return 恢复 TrapFrame 并 sret 到用户态。
    pub trap_frame_ptr: usize,
}

impl Task {
    /// 创建新的任务。
    ///
    /// 自动预分配标准文件描述符：
    /// - fd 0: stdin（RamFs 模拟）
    /// - fd 1: stdout（RamFs 模拟）
    /// - fd 2: stderr（RamFs 模拟）
    pub fn new(pid: usize, entry: usize, kstack_top: usize, ustack_top: usize) -> Self {
        let mut context = Context::zero_init();
        context.set_init_context(entry, kstack_top);

        // 预分配标准文件描述符
        let mut fd_table = FdTable::new();
        fd_table.open(alloc::boxed::Box::new(crate::fs::ramfs::RamFs::new())); // fd 0: stdin
        fd_table.open(alloc::boxed::Box::new(crate::fs::ramfs::RamFs::new())); // fd 1: stdout
        fd_table.open(alloc::boxed::Box::new(crate::fs::ramfs::RamFs::new())); // fd 2: stderr

        Self {
            pid,
            state: TaskState::Ready,
            context,
            kstack_top,
            ustack_top,
            exit_code: 0,
            fd_table,
            page_table_root: 0, // 内核任务不切换页表
            user_entry: 0, // 内核任务无用户入口
            trap_frame_ptr: 0, // 内核任务无陷阱帧
        }
    }

    /// 获取上下文指针
    pub fn context_ptr(&mut self) -> *mut Context {
        &mut self.context as *mut Context
    }

    /// 设置页表根物理页号
    ///
    /// 用于为用户任务设置独立的地址空间。
    /// 内核任务不需要调用此方法（page_table_root 默认为 0）。
    ///
    /// # 参数
    /// - `ppn`: 页表根物理页号（来自 `UserAddrSpace::root_ppn()`）
    pub fn set_page_table_root(&mut self, ppn: usize) {
        self.page_table_root = ppn;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_creation() {
        let task = Task::new(1, 0x8020_0000, 0x8040_0000, 0x8060_0000);
        assert_eq!(task.pid, 1);
        assert_eq!(task.state, TaskState::Ready);
        assert_eq!(task.context.ra, 0x8020_0000);
        assert_eq!(task.context.sp, 0x8040_0000);
        assert_eq!(task.exit_code, 0);
    }

    #[test]
    fn task_state_transitions() {
        let mut task = Task::new(1, 0, 0, 0);
        assert_eq!(task.state, TaskState::Ready);

        task.state = TaskState::Running;
        assert_eq!(task.state, TaskState::Running);

        task.state = TaskState::Waiting;
        assert_eq!(task.state, TaskState::Waiting);

        task.state = TaskState::Exited;
        assert_eq!(task.state, TaskState::Exited);
    }

    /// 验证任务创建时预分配标准文件描述符
    #[test]
    fn task_std_fds() {
        let task = Task::new(1, 0x8020_0000, 0x8040_0000, 0x8060_0000);

        // fd 0 = stdin, fd 1 = stdout, fd 2 = stderr
        assert!(task.fd_table.get(0).is_some(), "fd 0 (stdin) 应存在");
        assert!(task.fd_table.get(1).is_some(), "fd 1 (stdout) 应存在");
        assert!(task.fd_table.get(2).is_some(), "fd 2 (stderr) 应存在");

        // fd 3 不应存在
        assert!(task.fd_table.get(3).is_none(), "fd 3 不应存在");
    }
}
