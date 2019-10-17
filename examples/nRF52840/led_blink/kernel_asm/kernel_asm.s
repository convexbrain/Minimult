# LLD requires that the section flags are explicitly set here
.section .KernelAsm, "ax"
.global PendSV
.global ex_countup

# .type and .thumb_func are both required; otherwise its Thumb bit does not
# get set and an invalid vector table is generated
.type PendSV,%function
.thumb_func

PendSV:
    push    {r4, r5, r6, r7}
    mov     r4, r8
    mov     r5, r9
    mov     r6, r10
    mov     r7, r11
    push    {r4, r5, r6, r7}

    mov     r4, lr

    mov     r0, sp
    bl      save_sp
    mov     sp, r0
    bl      task_switch
    mov     sp, r0

    mov     lr, r4
    
    pop     {r4, r5, r6, r7}
    mov     r8, r4
    mov     r9, r5
    mov     r10, r6
    mov     r11, r7
    pop     {r4, r5, r6, r7}

    bx      lr


.type ex_countup,%function
.thumb_func

.ifdef V6

ex_countup:
    cpsid   i
    ldr     r1, [r0]
    add     r1, #1
    str     r1, [r0]
    cpsie   i
    bx      lr

.else

ex_countup:
    ldrex   r1, [r0]
    add     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     ex_countup
    bx      lr

.endif

