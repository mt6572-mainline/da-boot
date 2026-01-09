.syntax unified

.code 32

.global start
.section .text.start
start:
    ldr sp, =0x2006c00
    blx main
