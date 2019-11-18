# LLD requires that the section flags are explicitly set here
.section .text.minimult_asm, "ax"

# .type and .thumb_func are both required; otherwise its Thumb bit does not
# get set and an invalid vector table is generated
.global PendSV
.type PendSV,%function
.thumb_func

PendSV:
    push    {r0, lr}
    push    {r4, r5, r6, r7}
    mov     r4, r8
    mov     r5, r9
    mov     r6, r10
    mov     r7, r11
    push    {r4, r5, r6, r7}

    mov     r1, sp
.ifdef V8
    mrs     r2, msplim
.else
    mov     r2, #0
.endif
    bl      minimult_save_sp
.ifdef V8
    msr     msplim, r1
.endif
    mov     sp, r0
    mov     r2, r1
    mov     r1, r0
    bl      minimult_task_switch
    mov     sp, r0
.ifdef V8
    msr     msplim, r1
.endif

    pop     {r4, r5, r6, r7}
    mov     r8, r4
    mov     r9, r5
    mov     r10, r6
    mov     r11, r7
    pop     {r4, r5, r6, r7}
    pop     {r0, pc}

#####

.global minimult_ex_incr
.type minimult_ex_incr,%function
.thumb_func

.ifdef V6

minimult_ex_incr:
    cpsid   i
    ldr     r1, [r0]
    add     r1, #1
    str     r1, [r0]
    cpsie   i
    bx      lr

.else

minimult_ex_incr:
    ldrex   r1, [r0]
    add     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     minimult_ex_incr
    bx      lr

.endif

#####

.global minimult_ex_decr
.type minimult_ex_decr,%function
.thumb_func

.ifdef V6

minimult_ex_decr:
    cpsid   i
    ldr     r1, [r0]
    sub     r1, #1
    str     r1, [r0]
    cpsie   i
    bx      lr

.else

minimult_ex_decr:
    ldrex   r1, [r0]
    sub     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     minimult_ex_decr
    bx      lr

.endif

#####

.global minimult_ex_incr_ifgt0
.type minimult_ex_incr_ifgt0,%function
.thumb_func

.ifdef V6

minimult_ex_incr_ifgt0:
    cpsid   i
    ldr     r1, [r0]
    cmp     r1, #0
    bgt     minimult_ex_incr_ifgt0_true
    cpsie   i
    mov     r0, #0
    bx      lr
minimult_ex_incr_ifgt0_true:
    add     r1, #1
    str     r1, [r0]
    cpsie   i
    mov     r0, #1
    bx      lr

.else

minimult_ex_incr_ifgt0:
    ldrex   r1, [r0]
    cmp     r1, #0
    bgt     minimult_ex_incr_ifgt0_true
    mov     r0, #0
    bx      lr
minimult_ex_incr_ifgt0_true:
    add     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     minimult_ex_incr_ifgt0
    mov     r0, #1
    bx      lr

.endif

#####

.global minimult_ex_decr_if1
.type minimult_ex_decr_if1,%function
.thumb_func

.ifdef V6

minimult_ex_decr_if1:
    cpsid   i
    ldr     r1, [r0]
    cmp     r1, #1
    beq     minimult_ex_decr_if1_true
    cpsie   i
    mov     r0, #0
    bx      lr
minimult_ex_decr_if1_true:
    sub     r1, #1
    str     r1, [r0]
    cpsie   i
    mov     r0, #1
    bx      lr

.else

minimult_ex_decr_if1:
    ldrex   r1, [r0]
    cmp     r1, #1
    beq     minimult_ex_decr_if1_true
    mov     r0, #0
    bx      lr
minimult_ex_decr_if1_true:
    sub     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     minimult_ex_decr_if1
    mov     r0, #1
    bx      lr

.endif
