/// 任务控制块 (TCB)。
///
/// 包含一个任务的所有内核管理信息。
/// 这是操作系统中最核心的数据结构之一。
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
}

impl Task {
    /// 创建新的任务
    pub fn new(pid: usize, entry: usize, kstack_top: usize, ustack_top: usize) -> Self {
        let mut context = Context::zero_init();
        context.set_init_context(entry, kstack_top);

        Self {
            pid,
            state: TaskState::Ready,
            context,
            kstack_top,
            ustack_top,
            exit_code: 0,
        }
    }

    /// 获取上下文指针
    pub fn context_ptr(&mut self) -> *mut Context {
        &mut self.context as *mut Context
    }
}
