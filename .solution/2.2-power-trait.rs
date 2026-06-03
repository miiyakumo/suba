/// 电源管理 trait。
///
/// 提供关机和重启操作。
pub trait Power {
    /// 关机
    fn shutdown() -> !;

    /// 重启
    fn reboot() -> !;
}
