# LLD requires that the section flags are explicitly set here
.section .text.minimult_asm, "ax"

# .type and .thumb_func are both required; otherwise its Thumb bit does not
# get set and an invalid vector table is generated
.global PendSV
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
    bl      minimult_save_sp
    mov     sp, r0
    bl      minimult_task_switch
    mov     sp, r0

    mov     lr, r4
    
    pop     {r4, r5, r6, r7}
    mov     r8, r4
    mov     r9, r5
    mov     r10, r6
    mov     r11, r7
    pop     {r4, r5, r6, r7}

    bx      lr

#####

.global minimult_ex_cntup
.type minimult_ex_cntup,%function
.thumb_func

.ifdef V6

minimult_ex_cntup:
    cpsid   i
    ldr     r1, [r0]
    add     r1, #1
    str     r1, [r0]
    cpsie   i
    bx      lr

.else

minimult_ex_cntup:
    ldrex   r1, [r0]
    add     r1, #1
    strex   r2, r1, [r0]
    cmp     r2, #0
    bne     minimult_ex_cntup
    bx      lr

.endif

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
