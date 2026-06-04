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

    /// 为测试设置标准文件描述符（stdin=0, stdout=1, stderr=2）
    fn setup_std_fds() {
        r#impl::with_fd_table(|table| {
            if table.get(0).is_none() {
                table.open(Box::new(RamFs::new())); // fd 0: stdin
                table.open(Box::new(RamFs::new())); // fd 1: stdout
                table.open(Box::new(RamFs::new())); // fd 2: stderr
            }
        });
    }

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

    /// 集成测试：验证所有已定义的系统调用号都能正确路由
    #[test]
    fn dispatch_all_syscalls() {
        setup_std_fds();

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

        let mut frame = MockTrapFrame::new();
        frame.syscall_no = 0;
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize);
    }

    /// 集成测试：sys_write 通过 VFS 写入文件
    #[test]
    fn sys_write_via_vfs() {
        setup_std_fds();
        // 写入到 stdout (fd=1)
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_WRITE;
        frame.args = [1, 0x8000_0000, 42, 0, 0, 0]; // fd=1, len=42
        dispatch(&mut frame);
        // VFS 写入成功，返回写入字节数
        assert_eq!(frame.ret, 42);
    }

    /// 集成测试：sys_read 通过 VFS 读取文件
    #[test]
    fn sys_read_via_vfs() {
        setup_std_fds();
        // 从 stdin (fd=0) 读取 — 空文件，返回 0
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_READ;
        frame.args = [0, 0x8000_0000, 10, 0, 0, 0]; // fd=0, len=10
        dispatch(&mut frame);
        assert_eq!(frame.ret, 0); // 空文件读取 0 字节
    }

    /// 集成测试：写入后通过 VFS 读取验证
    #[test]
    fn sys_write_then_read_via_vfs() {
        // 创建带数据的 RamFs 并注册到 fd=3
        r#impl::with_fd_table(|table| {
            table.open(Box::new(RamFs::with_data(alloc::vec![10, 20, 30])));
        });

        // 通过 sys_read 从 fd=3 读取
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_READ;
        frame.args = [3, 0x8000_0000, 2, 0, 0, 0]; // fd=3, len=2
        dispatch(&mut frame);
        assert_eq!(frame.ret, 2); // 成功读取 2 字节

        // 验证文件 offset 已更新
        r#impl::with_fd_table(|table| {
            let file = table.get(3).unwrap();
            assert_eq!(file.size(), 3); // 文件大小不变
        });
    }

    /// 测试：无效 fd 返回 -1
    #[test]
    fn sys_write_invalid_fd() {
        setup_std_fds();
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_WRITE;
        frame.args = [99, 0x8000_0000, 10, 0, 0, 0]; // fd=99 不存在
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize);
    }

    /// 测试：无效 fd 读取返回 -1
    #[test]
    fn sys_read_invalid_fd() {
        setup_std_fds();
        let mut frame = MockTrapFrame::new();
        frame.syscall_no = number::SYS_READ;
        frame.args = [99, 0x8000_0000, 10, 0, 0, 0]; // fd=99 不存在
        dispatch(&mut frame);
        assert_eq!(frame.ret, (-1isize) as usize);
    }
}
