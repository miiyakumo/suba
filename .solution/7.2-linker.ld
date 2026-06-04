/* 链接脚本 — 定义 RISC-V 内核内存布局
 *
 * 教学要点：
 * - BASE_ADDRESS = 0x80200000 是 QEMU virt 机器的内核加载地址
 *   (DRAM 起始于 0x8000_0000，前 0x20_0000 留给 OpenSBI)
 * - .text.entry 段必须放在最前面，保证 _start 是入口点
 * - 各段之间 4K 对齐，便于后续页表映射
 * - 符号 skernel/ekernel 标记内核整体范围
 */

OUTPUT_ARCH(riscv)
ENTRY(_start)
BASE_ADDRESS = 0x80200000;

SECTIONS
{
    . = BASE_ADDRESS;
    skernel = .;

    stext = .;
    .text : {
        *(.text.entry)
        *(.text .text.*)
    }

    . = ALIGN(4K);
    etext = .;
    srodata = .;
    .rodata : {
        *(.rodata .rodata.*)
        *(.srodata .srodata.*)
    }

    . = ALIGN(4K);
    erodata = .;
    sdata = .;
    .data : {
        *(.data .data.*)
        *(.sdata .sdata.*)
    }

    . = ALIGN(4K);
    edata = .;
    .bss : {
        *(.bss.stack)
        sbss = .;
        *(.bss .bss.*)
        *(.sbss .sbss.*)
    }

    . = ALIGN(4K);
    ebss = .;
    ekernel = .;

    /DISCARD/ : {
        *(.eh_frame)
    }
}
