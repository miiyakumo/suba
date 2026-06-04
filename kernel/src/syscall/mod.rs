//! # 系统调用 (syscall)
//!
//! 本模块实现了系统调用的分发和处理。
//!
//! ## 教学概念
//! - **系统调用**：用户程序请求内核服务的唯一合法通道
//! - **系统调用号**：每个系统调用有一个唯一编号
//! - **分发 (dispatch)**：根据系统调用号路由到对应的处理函数
//! - **参数传递**：通过寄存器 a0-a5 传递参数，a7 传递系统调用号
//!
//! ## 教学故事
//! "用户程序不能直接访问硬件——它必须通过系统调用'敲门'，
//!  内核检查请求是否合法，然后代为执行。"

pub mod r#impl;

// TODO: 定义系统调用号
// 参考 .solution/5.1-syscall-numbers.rs
//
// 提示：参考 Linux RISC-V 系统调用号

/// 系统调用号定义
///
/// 参考 Linux RISC-V 系统调用号，保持兼容性。
pub mod number {
    /// 退出当前任务
    pub const SYS_EXIT: usize = 93;
    /// 让出 CPU
    pub const SYS_YIELD: usize = 124;
    /// 读取文件描述符
    pub const SYS_READ: usize = 63;
    /// 写入文件描述符
    pub const SYS_WRITE: usize = 64;
    /// 打开文件
    pub const SYS_OPEN: usize = 1024;
    /// 关闭文件描述符
    pub const SYS_CLOSE: usize = 57;
    /// 调整堆大小
    pub const SYS_SBRK: usize = 214;
    /// 获取 PID
    pub const SYS_GETPID: usize = 172;
}

// TODO: 实现系统调用分发
// 参考 .solution/5.2-syscall-dispatch.rs
//
// 提示：match 系统调用号，调用对应处理函数，未知调用返回 -1

/// 系统调用分发。
///
/// 根据 `SyscallFrame` 中的系统调用号，路由到对应的处理函数。
pub fn dispatch<F: crate::arch::SyscallFrame>(frame: &mut F) {
    let ret = match frame.syscall_id() {
        number::SYS_WRITE => r#impl::sys_write(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_READ => r#impl::sys_read(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_EXIT => r#impl::sys_exit(frame.arg0() as i32),
        number::SYS_YIELD => r#impl::sys_yield(),
        number::SYS_SBRK => r#impl::sys_sbrk(frame.arg0() as isize),
        number::SYS_GETPID => r#impl::sys_getpid(),
        _ => {
            // 未知系统调用
            -1isize as usize
        }
    };
    frame.set_ret(ret);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arch::mock::MockTrapFrame;

    #[test]
    fn dispatch_unknown_syscall() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = 9999; // 不存在的系统调用
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize);
    }

    #[test]
    fn syscall_numbers() {
        assert_eq!(number::SYS_EXIT, 93);
        assert_eq!(number::SYS_WRITE, 64);
        assert_eq!(number::SYS_READ, 63);
    }

    #[test]
    fn dispatch_yield() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_YIELD;
        dispatch(&mut frame);
        // sys_yield 始终返回 0
        assert_eq!(frame.ret, 0);
    }

    #[test]
    fn dispatch_sbrk() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_SBRK;
        frame.args[0] = 4096; // increment = 4096
        dispatch(&mut frame);
        // Mock: 返回固定的堆顶地址
        assert_eq!(frame.ret, 0x8080_0000);
    }
}
