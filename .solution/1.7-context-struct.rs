/// 上下文切换时保存的寄存器。
///
/// 仅包含 callee-saved 寄存器（ra, sp, s0-s11），
/// 这是 Voluntary Context Switch 所需的最小集合。
///
/// ## 教学概念
/// 上下文切换不需要保存所有寄存器——caller-saved 寄存器由编译器
/// 在函数调用时自动保存到栈上，只需保存 callee-saved 寄存器。
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Context {
    /// 返回地址
    pub ra: usize,
    /// 栈指针
    pub sp: usize,
    /// s0-s11 callee-saved 寄存器
    pub s: [usize; 12],
}

impl Context {
    /// 创建全零初始化的上下文
    pub const fn zero_init() -> Self {
        Self {
            ra: 0,
            sp: 0,
            s: [0; 12],
        }
    }

    /// 设置初始上下文（入口点和栈顶）
    pub fn set_init_context(&mut self, entry: usize, stack_top: usize) {
        self.ra = entry;
        self.sp = stack_top;
    }
}
