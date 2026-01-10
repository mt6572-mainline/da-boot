.syntax unified
.code 32

.section .text.start
.global start

start:
    adr r4, start               // where am i

    movw r2, :lower16:(_rel_dyn_start - 1f)
    movt r2, :upper16:(_rel_dyn_start - 1f)
1:  add r2, r2, pc

    movw r3, :lower16:(_rel_dyn_end - 2f)
    movt r3, :upper16:(_rel_dyn_end - 2f)
2:  add r3, r3, pc

.L_relocate_loop:
    cmp r2, r3
    bge startup

    ldr r0, [r2], #4            // offset
    ldr r1, [r2], #4            // data

    and r1, r1, #0xFF
    cmp r1, #23                 // R_ARM_RELATIVE
    bne .L_relocate_loop

    add r0, r0, r4
    ldr r5, [r0]
    add r5, r5, r4              // relocate
    str r5, [r0]

    b .L_relocate_loop

startup:
    bl is_bootrom

    cmp r0, #1
    beq .L_stack_sram

.L_stack_dram:
    movw sp, #:lower16:0x90000000
    movt sp, #:upper16:0x90000000
    b .L_jump_main

.L_stack_sram:
    movw sp, #:lower16:0x2006c00
    movt sp, #:upper16:0x2006c00

.L_jump_main:
    bl main
