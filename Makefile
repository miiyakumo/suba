DOCKER_TAG ?= suba:latest
.PHONY: fmt run build clean clean-all

fmt:
	cd os && cargo fmt

# 构建内核
build:
	cd os && cargo build

# 运行内核
run:
	cd os && cargo run

# 清理 OS 构建产物
clean:
	cd os && cargo clean
