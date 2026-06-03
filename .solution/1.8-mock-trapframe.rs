/// Mock 陷阱帧。
///
/// 保存系统调用参数和返回值，用于测试系统调用分发逻辑。
#[derive(Debug, Clone, Copy)]
pub struct MockTrapFrame {
    /// 系统调用号
    pub syscall_no: usize,
    /// 参数 0-5
    pub args: [usize; 6],
    /// 返回值
    pub ret: usize,
    /// 程序计数器
    pub pc: usize,
    /// 栈指针
    pub sp: usize,
}

impl MockTrapFrame {
    /// 创建新的 Mock 陷阱帧
    pub fn new() -> Self {
        Self {
            syscall_no: 0,
            args: [0; 6],
            ret: 0,
            pc: 0,
            sp: 0,
        }
    }
}

impl HwTrapFrame for MockTrapFrame {
    fn zero_init() -> Self {
        Self::new()
    }

    fn set_kernel_trap_frame(&mut self, entry: usize, _terminal: usize, kernel_sp: usize) {
        self.pc = entry;
        self.sp = kernel_sp;
    }

    fn get_sp(&self) -> usize {
        self.sp
    }

    fn set_sp(&mut self, val: usize) {
        self.sp = val;
    }

    fn set_a0(&mut self, val: usize) {
        self.args[0] = val;
    }

    fn set_a1(&mut self, val: usize) {
        self.args[1] = val;
    }

    fn set_a2(&mut self, val: usize) {
        self.args[2] = val;
    }

    fn set_ra(&mut self, _val: usize) {
        // Mock: 无操作
    }

    fn set_sepc(&mut self, pc: usize) {
        self.pc = pc;
    }

    fn get_sepc(&self) -> usize {
        self.pc
    }
}

impl SyscallFrame for MockTrapFrame {
    fn syscall_id(&self) -> usize {
        self.syscall_no
    }

    fn arg0(&self) -> usize {
        self.args[0]
    }
    fn arg1(&self) -> usize {
        self.args[1]
    }
    fn arg2(&self) -> usize {
        self.args[2]
    }
    fn arg3(&self) -> usize {
        self.args[3]
    }
    fn arg4(&self) -> usize {
        self.args[4]
    }
    fn arg5(&self) -> usize {
        self.args[5]
    }

    fn set_ret(&mut self, val: usize) {
        self.ret = val;
    }
}
