/*
 * suba 内核链接脚本 — RISC-V 64 位
 *
 * 内存布局 (QEMU virt 机器):
 *   0x8000_0000        DRAM 起始
 *   0x8020_0000        内核加载地址 (BASE_ADDRESS)
 *   0x8800_0000        DRAM 结束 (128MB)
 *
 * 段排列顺序:
 *   .text   — 代码（含 .text.entry 入口段）
 *   .rodata — 只读数据
 *   .data   — 已初始化的可写数据
 *   .bss    — 未初始化数据（运行时清零）
 *
 * 链接脚本定义的符号（entry.S 和 Rust 代码中使用）:
 *   skernel / ekernel  — 内核镜像起止
 *   stext / etext      — .text 段起止
 *   sbss / ebss        — .bss 段起止（BSS 清零用）
 */

OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    /* ---- 代码段 ---- */
    stext = .;
    .text : {
        *(.text.entry)       /* entry.S 入口代码，必须最先 */
        *(.text .text.*)
    }

    /* ---- 只读数据段 ---- */
    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    /* ---- 已初始化数据段 ---- */
    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    /* ---- BSS 段（未初始化数据，运行时清零） ---- */
    . = ALIGN(4K);
    edata = .;
    .bss : {
        *(.bss.stack)        /* 内核栈空间（entry.S 中定义） */
        sbss = .;            /* BSS 清零起点 */
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;                /* BSS 清零终点 */
    ekernel = .;             /* 内核镜像结束 */

    /DISCARD/ : {
        *(.eh_frame)         /* 丢弃异常帧（no_std 不需要） */
    }
}
