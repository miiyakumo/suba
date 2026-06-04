// .solution/9.1-uart-registers.rs — UART MMIO 寄存器定义参考实现
//
// 本文件是 feature 9.1 的参考实现。
// 学生应在 os/src/driver/uart.rs 中实现 UART MMIO 寄存器定义。
//
// ## 核心实现
//
// 1. UART 基地址常量 (UART0_BASE = 0x1000_0000)
// 2. UartRegister 枚举定义寄存器偏移量 (RBR/THR, IER, IIR/FCR, LCR, MCR, LSR, MSR, SCR)
// 3. LsrFlags 结构体定义 LSR 状态位 (DATA_READY, THR_EMPTY, TRANSMITTER_EMPTY)
// 4. Uart 结构体提供 volatile 读写方法 (read_reg, write_reg)
// 5. putc: 等待 THRE=1 后写入 THR
// 6. getc: 检查 DR=1 后读取 RBR
// 7. 编译期断言验证寄存器偏移量
//
// ## 关键教学点
//
// - MMIO: 通过读写内存地址来控制硬件
// - volatile: 防止编译器优化硬件访问
// - 忙等待: 轮询状态寄存器直到条件满足
// - NS16550A: 经典 UART 芯片，QEMU virt 机器模拟
