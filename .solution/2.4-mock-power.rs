/// Mock 电源管理实现。
pub struct MockPower;

impl Power for MockPower {
    fn shutdown() -> ! {
        // 在测试中，shutdown 通过 panic 来"终止"
        panic!("MockPower::shutdown called");
    }

    fn reboot() -> ! {
        panic!("MockPower::reboot called");
    }
}
