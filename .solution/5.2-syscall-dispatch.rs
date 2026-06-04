//! # 系统调用分发框架
//!
//! 参考实现：根据系统调用号路由到对应处理函数。

use super::number;

/// 系统调用分发。
///
/// 根据 `SyscallFrame` 中的系统调用号，路由到对应的处理函数。
/// 未知系统调用返回 -1。
pub fn dispatch<F: crate::arch::SyscallFrame>(frame: &mut F) {
    let ret = match frame.syscall_id() {
        number::SYS_WRITE => super::r#impl::sys_write(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_READ => super::r#impl::sys_read(frame.arg0(), frame.arg1(), frame.arg2()),
        number::SYS_EXIT => super::r#impl::sys_exit(frame.arg0() as i32),
        number::SYS_YIELD => super::r#impl::sys_yield(),
        number::SYS_SBRK => super::r#impl::sys_sbrk(frame.arg0() as isize),
        number::SYS_GETPID => super::r#impl::sys_getpid(),
        _ => {
            // 未知系统调用
            -1isize as usize
        }
    };
    frame.set_ret(ret);
}
