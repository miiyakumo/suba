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
    use crate::fs::ramfs::RamFs;
    use alloc::boxed::Box;

    #[test]
    fn dispatch_unknown_syscall() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = 9999;
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
        assert_eq!(frame.ret, 0);
    }

    #[test]
    fn dispatch_sbrk() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_SBRK;
        frame.args[0] = 4096;
        dispatch(&mut frame);
        assert_eq!(frame.ret, 0x8080_0000);
    }

    #[test]
    fn dispatch_getpid() {
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_GETPID;
        dispatch(&mut frame);
        assert_eq!(frame.ret, 1);
    }

    /// 集成测试：dispatch 路由 + VFS 操作。
    ///
    /// 由于全局 FD_TABLE 在并行测试间共享，在一个原子测试中覆盖：
    /// - 所有已定义系统调用号的路由验证
    /// - sys_write/sys_read 通过 dispatch → FdTable → VfsFile
    /// - 无效 fd 返回 -1
    #[test]
    fn dispatch_all_and_vfs_integration() {
        // 设置 stdin/stdout/stderr
        r#impl::with_fd_table(|table| {
            table.clear();
            table.open(Box::new(RamFs::new())); // fd 0: stdin
            table.open(Box::new(RamFs::new())); // fd 1: stdout
            table.open(Box::new(RamFs::new())); // fd 2: stderr
        });

        // --- 已知系统调用路由验证 ---
        let test_cases: &[(usize, &str, [usize; 6])] = &[
            (number::SYS_WRITE, "write",   [1, 0x8000_0000, 10, 0, 0, 0]),
            (number::SYS_READ,  "read",    [0, 0x8000_0000, 10, 0, 0, 0]),
            (number::SYS_EXIT,  "exit",    [0, 0, 0, 0, 0, 0]),
            (number::SYS_YIELD, "yield",   [0, 0, 0, 0, 0, 0]),
            (number::SYS_SBRK,  "sbrk",    [4096, 0, 0, 0, 0, 0]),
            (number::SYS_GETPID,"getpid",  [0, 0, 0, 0, 0, 0]),
        ];

        for &(no, name, args) in test_cases {
            let mut frame = MockTrapFrame::new();
            frame.syscall_no = no;
            frame.args = args;
            dispatch(&mut frame);
            assert_ne!(
                frame.ret,
                (-1isize) as usize,
                "syscall {} ({}) was treated as unknown",
                name,
                no
            );
        }

        // --- 未知系统调用 ---
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = 0;
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize);

        // --- sys_write 写入 stdout ---
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_WRITE;
        frame.args = [1, 0x8000_0000, 42, 0, 0, 0];
        dispatch(&mut frame);
        assert_eq!(frame.ret, 42, "sys_write 到 stdout 应返回写入字节数");

        // --- sys_read 从 stdin ---
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_READ;
        frame.args = [0, 0x8000_0000, 10, 0, 0, 0];
        dispatch(&mut frame);
        assert_eq!(frame.ret, 0, "sys_read 空 stdin 应返回 0");

        // --- 无效 fd ---
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_WRITE;
        frame.args = [99, 0x8000_0000, 10, 0, 0, 0];
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize, "无效 fd 写入应返回 -1");

        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_READ;
        frame.args = [99, 0x8000_0000, 10, 0, 0, 0];
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize, "无效 fd 读取应返回 -1");
    }
}
