// Solution: 11.3 - 加载用户 ELF 程序
//
// 本文件展示如何嵌入并加载用户 ELF 程序。
// 完整代码位于 os/src/main.rs。

// ============================================================================
// 嵌入用户程序
// ============================================================================

// 1. 创建最小 RISC-V 用户程序（user_init.S）：
//
//    .global _start
//    _start:
//        li a7, 93       # sys_exit 系统调用号
//        li a0, 0        # 退出码 0
//        ecall            # 触发系统调用
//
// 2. 交叉编译：
//    riscv64-unknown-elf-as -o user_init.o user_init.S
//    riscv64-unknown-elf-ld -o user_init.elf user_init.o -Ttext 0x80001000
//    riscv64-unknown-elf-objcopy -O binary user_init.elf user_init.bin
//
// 3. 嵌入内核：
//    let init_elf = include_bytes!("user_init.bin");

// ============================================================================
// 加载流程
// ============================================================================

// 在 rust_main 的 Step 9 中：
//
//   let init_elf = include_bytes!("user_init.bin");
//
//   match arch::riscv64::load_elf_to_space(init_elf) {
//       Ok((user_space, entry, stack_top)) => {
//           // 1. ELF 解析成功，段已加载到用户页表
//           // 2. 用户栈已映射 (8MB)
//           // 3. 激活用户页表
//           unsafe { user_space.activate(); }
//
//           // 4. 切换到用户态
//           enter_user_mode(entry, stack_top, kernel_sp, 0);
//       }
//       Err(_) => {
//           uart_puts("[boot] WARN: init ELF load failed\n");
//       }
//   }

// ============================================================================
// 教学概念：ELF 加载的完整路径
// ============================================================================

// ELF 加载涉及多个内核子系统的协作：
//
//   include_bytes!          →  嵌入 ELF 数据到内核二进制
//   parse_elf_header()      →  验证 ELF 魔数、架构、类型
//   parse_program_headers() →  提取 PT_LOAD 段信息
//   UserAddrSpace::new()    →  创建用户页表
//   load_segments()         →  分配物理帧，复制段数据，映射到用户 VA
//   map_user_stack()        →  分配 8MB 用户栈
//   activate()              →  写入 satp 切换到用户页表
//   enter_user_mode()       →  设置 TrapFrame，sret 到用户态
//
// 这个流程展示了内核如何将"静态文件"转化为"正在执行的程序"。
