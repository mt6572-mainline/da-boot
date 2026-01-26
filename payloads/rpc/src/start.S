.syntax unified
.code 32

.section .text.start
.global start

start:
    adr r4, start               // where am i

    bl is_bootrom
    cmp r0, #1
    beq .L_relocate

    movw r2, #:lower16:0x9c000000
    movt r2, #:upper16:0x9c000000
    cmp r4, r2
    beq .L_relocate

.L_copy:
    mov r0, r4

    movw r1, :lower16:_image_end
    movt r1, :upper16:_image_end
    add r1, r1, r4

    mov r5, r2                  // jump addr

.L_copy_loop:
    ldmia r0!, {{r6-r9}}
    stmia r2!, {{r6-r9}}
    cmp r0, r1
    blt .L_copy_loop

    mov r0, #0
    mcr p15, 0, r0, c7, c10, 4  // dsb
    mcr p15, 0, r0, c7, c5, 0   // invalidate icache
    mcr p15, 0, r0, c7, c5, 6   // Invalidate bp
    mcr p15, 0, r0, c7, c10, 4  // dsb
    isb sy

    bx r5

.L_relocate:
    movw r2, :lower16:_rel_dyn_start
    movt r2, :upper16:_rel_dyn_start
    add r2, r2, r4

    movw r3, :lower16:_rel_dyn_end
    movt r3, :upper16:_rel_dyn_end
    add r3, r3, r4

.L_relocate_loop:
    cmp r2, r3
    bge startup

    ldr r0, [r2], #4            // offset
    ldr r1, [r2], #4            // data

    and r1, r1, #0xFF
    cmp r1, #23                 // R_ARM_RELATIVE
    bne .L_relocate_loop

    add r0, r0, r4              // addr
    ldr r5, [r0]
    add r5, r5, r4
    str r5, [r0]

    b .L_relocate_loop

startup:
    bl is_bootrom

    cmp r0, #1
    beq .L_stack_sram

.L_stack_dram:
    movw sp, #:lower16:0x8ffffff8
    movt sp, #:upper16:0x8ffffff8
    b .L_jump_main

.L_stack_sram:
    movw sp, #:lower16:0x2006c00
    movt sp, #:upper16:0x2006c00

.L_jump_main:
    bl main
