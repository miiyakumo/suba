#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heap_size_constant() {
        assert_eq!(HEAP_SIZE, 0x20_0000); // 2MB
    }

    #[test]
    fn heap_init_sets_initialized() {
        // SAFETY: 测试环境单线程，且使用 mock 地址
        unsafe { init_heap(0x1000_0000, HEAP_SIZE) };
        assert!(is_initialized());
    }
}
