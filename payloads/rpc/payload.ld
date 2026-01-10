OUTPUT_FORMAT("elf32-littlearm", "elf32-bigarm", "elf32-littlearm")
OUTPUT_ARCH(arm)

ENTRY(start)

SECTIONS
{
    . = 0x0;

    .text : {
        KEEP(*(.text.start))
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    .got : {
        _got_start = .;
        *(.got)
        *(.got.plt)
        _got_end = .;
    }

    .rel.dyn : ALIGN(4) {
        _rel_dyn_start = .;
        *(.rel.dyn)
        _rel_dyn_end = .;
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        _bss_start = .;
        *(.bss .bss.*)
        _bss_end = .;
    }
}
