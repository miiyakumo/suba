DOCKER_TAG ?= suba:latest
.PHONY: docker build_docker fmt run build clean clean-all

docker:
	docker run --rm -it -v ${PWD}:/mnt -w /mnt --name comix ${DOCKER_TAG} bash

build_docker:
	docker build -t ${DOCKER_TAG} --target build .

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
