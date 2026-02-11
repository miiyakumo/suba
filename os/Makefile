# ===============================================
# 环境变量
# ===============================================
PROJECT_DIR := target/riscv64gc-unknown-none-elf/debug/os
QEMU_RUNNER := ./qemu-run.sh
GDB_BIN := riscv64-unknown-elf-gdb

# ===============================================
# 核心目标
# ===============================================
.PHONY: run test debug gdb dtb

# 目标 1: run (默认启动 QEMU)
run:
	@echo "Running $(PROJECT_DIR)..."
	@cargo build && $(QEMU_RUNNER) $(PROJECT_DIR) run

# 目标 2: test (显式运行测试)
test:
	@cargo test

# 目标 3: gdb (连接到正在运行的 QEMU)
gdb:
	@echo "Starting GDB client and connecting to QEMU on tcp:1234..."
	$(GDB_BIN) \
		-ex "file $(PROJECT_DIR)" \
		-ex "target remote :1234" \
        -ex "b *0x80200000" \
        -ex "c"

# 目标 4: debug (一键启动，QEMU在前台，需要另一个终端运行gdb)
debug:
	@cargo build
	@echo "======================================================="
	@echo "在新的终端窗口中运行 'make gdb' 来连接到 QEMU。"
	@echo "QEMU 正在等待 GDB 连接..."
	@echo "======================================================="
	@$(QEMU_RUNNER) $(PROJECT_DIR) gdb &

# 目标 5: dtb (生成设备树二进制文件) XXX: 没有保证单一数据源
dtb:
	@echo "Generating device tree binary (DTB) file..."
	@qemu-system-riscv64 -machine virt,dumpdtb=virt.dtb && dtc virt.dtb