fmt:
	cd os && cargo fmt

# 构建内核
build:
	cd os && cargo build

# 运行内核
run:
	cd os && cargo run

# 快速代码风格检查
quick_check_style:
	cargo clippy -- -A warnings
	cargo fmt --all -- --check