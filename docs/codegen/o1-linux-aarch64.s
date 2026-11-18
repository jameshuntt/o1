== _RNvNtCsoSpHl677Cf_8o1_linux3uds10recv_frame
	sub	sp, sp, #112
	stp	x29, x30, [sp, #32]
	str	x25, [sp, #48]
	stp	x24, x23, [sp, #64]
	stp	x22, x21, [sp, #80]
	stp	x20, x19, [sp, #96]
	add	x29, sp, #32
	mov	x20, x1
	mov	x19, x8
	add	x24, x1, #56
	mov	w21, w0
	mov	w25, #4
	b	.LBB0_2
.LBB0_1:
	cmn	x1, #4
	b.ne	.LBB0_17
.LBB0_2:
	ldr	w9, [x20]
	subs	w8, w9, #4
	csel	w8, w25, w8, lo
	cmp	w8, #1
	b.gt	.LBB0_6
	cbnz	w8, .LBB0_8
	ldrb	w0, [x20, #4]
	cmp	x0, #4
	b.hi	.LBB0_31
	sub	x22, x25, x0
	add	x23, x24, x0
	b	.LBB0_11
.LBB0_6:
	cmp	w8, #2
	b.ne	.LBB0_19
	ldp	x23, x8, [x20, #24]
	ldr	x9, [x20, #16]
	cmp	x8, x9
	csel	x22, x8, x9, lo
	b	.LBB0_11
.LBB0_8:
	ldp	x1, x0, [x20, #8]
	ldr	x2, [x20, #32]
	subs	x22, x1, x0
	b.lo	.LBB0_30
	cmp	x1, x2
	b.hi	.LBB0_30
	ldr	x8, [x20, #24]
	add	x23, x8, x0
.LBB0_11:
	mov	x0, x21
	mov	x1, x23
	mov	w8, #63
	mov	x2, x22
	//APP
	svc	#0
	//NO_APP
	mov	x1, x0
	cmn	x0, #1, lsl #12
	b.hi	.LBB0_1
	add	x8, sp, #8
	mov	x0, x20
	bl	_RNvMs0_NtCsgrh34BrZ0r5_7o1_core5frameNtB5_12FrameDecoder6filled
	ldr	w8, [sp, #8]
	cmn	w8, #1
	b.ne	.LBB0_21
	ldr	x9, [sp, #16]
	cbz	x9, .LBB0_2
	ldr	x8, [sp, #24]
	cmp	x9, #1
	b.eq	.LBB0_23
	cmp	x9, #2
	b.ne	.LBB0_24
	mov	w9, #1
	stp	x9, x8, [x19, #8]
	mov	w9, #65535
	b	.LBB0_26
.LBB0_17:
	and	w8, w1, #0xffff
	mov	w9, #65525
	cmp	w8, w9
	b.ne	.LBB0_22
	mov	w8, #3
	b	.LBB0_25
.LBB0_19:
	cmp	w8, #3
	b.ne	.LBB0_27
	b	.LBB0_28
.LBB0_21:
	ldr	w10, [sp, #12]
	ldur	q0, [sp, #16]
	mov	w9, #1
	strh	w9, [x19]
	stp	w8, w10, [x19, #8]
	str	q0, [x19, #16]
	b	.LBB0_29
.LBB0_22:
	neg	w8, w1
	strh	wzr, [x19]
	strh	w8, [x19, #2]
	b	.LBB0_29
.LBB0_23:
	stp	xzr, x8, [x19, #8]
	mov	w8, #65535
	strh	w8, [x19]
	b	.LBB0_29
.LBB0_24:
	mov	w8, #2
.LBB0_25:
	mov	w9, #65535
	str	x8, [x19, #8]
.LBB0_26:
	strh	w9, [x19]
	b	.LBB0_29
.LBB0_27:
	ldp	x23, x22, [x20, #8]
	mov	w8, w9
	ldr	w10, [x20, #4]
.LBB0_28:
	mov	w9, #1
	stp	w8, w10, [x19, #8]
	strh	w9, [x19]
	stp	x23, x22, [x19, #16]
.LBB0_29:
	ldp	x20, x19, [sp, #96]
	ldr	x25, [sp, #48]
	ldp	x22, x21, [sp, #80]
	ldp	x24, x23, [sp, #64]
	ldp	x29, x30, [sp, #32]
	add	sp, sp, #112
	ret
.LBB0_30:
	adrp	x3, .Lanon.bfc85eb2e45f2c79feca98a593246e11.2
	add	x3, x3, :lo12:.Lanon.bfc85eb2e45f2c79feca98a593246e11.2
	bl	_RNvNtNtCsfo0Ls1ZyrQZ_4core5slice5index16slice_index_fail
.LBB0_31:
	adrp	x3, .Lanon.bfc85eb2e45f2c79feca98a593246e11.1
	add	x3, x3, :lo12:.Lanon.bfc85eb2e45f2c79feca98a593246e11.1
	mov	w1, #4
	mov	w2, #4
	bl	_RNvNtNtCsfo0Ls1ZyrQZ_4core5slice5index16slice_index_fail

== _RNvNtCsoSpHl677Cf_8o1_linux3uds10send_frame
	sub	sp, sp, #112
	stp	x29, x30, [sp, #96]
	add	x29, sp, #96
	mov	x10, x1
	ldr	x1, [x1, #8]
	ldr	x17, [x10, #16]
	add	x9, x1, #4
	cmp	x9, x17
	mov	x9, x8
	b.ne	.LBB1_2
.LBB1_1:
	mov	w8, #65535
	strb	wzr, [x9, #2]
	strh	w8, [x9]
	ldp	x29, x30, [sp, #96]
	add	sp, sp, #112
	ret
.LBB1_2:
	add	x12, x10, #24
	add	x13, sp, #40
	mov	w11, w0
	mov	w14, #4
	add	x15, sp, #8
	mov	w16, #2
	b	.LBB1_4
.LBB1_3:
	cmp	x8, x17
	b.eq	.LBB1_1
.LBB1_4:
	subs	x0, x17, #4
	b.hs	.LBB1_6
	ldr	x18, [x10]
	sub	x8, x14, x17
	add	x17, x12, x17
	b	.LBB1_8
.LBB1_6:
	subs	x2, x1, x0
	b.lo	.LBB1_19
	ldr	x17, [x10]
	mov	x8, xzr
	mov	x1, x2
	add	x18, x17, x0
	mov	w17, #1
.LBB1_8:
	stp	x17, x8, [sp, #8]
	mov	x0, x11
	mov	w8, #211
	stp	x18, x1, [sp, #24]
	add	x1, sp, #40
	mov	w2, #16384
	str	xzr, [sp, #40]
	str	wzr, [sp, #48]
	stp	x15, x16, [sp, #56]
	stp	xzr, xzr, [x13, #32]
	stur	wzr, [x13, #48]
	//APP
	svc	#0
	//NO_APP
	cmn	x0, #1, lsl #12
	b.ls	.LBB1_11
	cmn	x0, #4
	b.ne	.LBB1_14
	ldp	x1, x17, [x10, #8]
	add	x8, x1, #4
	b	.LBB1_3
.LBB1_11:
	cbz	x0, .LBB1_16
	ldp	x1, x18, [x10, #8]
	add	x8, x1, #4
	sub	x17, x8, x18
	cmp	x0, x17
	b.hi	.LBB1_18
	add	x17, x18, x0
	str	x17, [x10, #16]
	b	.LBB1_3
.LBB1_14:
	and	w8, w0, #0xffff
	mov	w10, #65525
	cmp	w8, w10
	b.ne	.LBB1_17
	mov	w8, #1
	mov	w10, #65535
	strb	w8, [x9, #2]
	strh	w10, [x9]
	ldp	x29, x30, [sp, #96]
	add	sp, sp, #112
	ret
.LBB1_16:
	mov	w8, #3
	strh	w8, [x9]
	ldp	x29, x30, [sp, #96]
	add	sp, sp, #112
	ret
.LBB1_17:
	neg	w8, w0
	strh	wzr, [x9]
	strh	w8, [x9, #2]
	ldp	x29, x30, [sp, #96]
	add	sp, sp, #112
	ret
.LBB1_18:
	mov	w8, #2
	mov	w10, #1
	str	x17, [x9, #24]
	strh	w8, [x9]
	stp	x10, x0, [x9, #8]
	ldp	x29, x30, [sp, #96]
	add	sp, sp, #112
	ret
.LBB1_19:
	adrp	x3, .Lanon.bfc85eb2e45f2c79feca98a593246e11.3
	add	x3, x3, :lo12:.Lanon.bfc85eb2e45f2c79feca98a593246e11.3
	mov	x2, x1
	bl	_RNvNtNtCsfo0Ls1ZyrQZ_4core5slice5index16slice_index_fail

== _RNvNtCsoSpHl677Cf_8o1_linux5futex16wait_while_equal
	mov	w9, w1
	mov	x10, x0
	mov	w11, #65525
	mov	w2, w1
	mov	w12, #65532
	mov	w13, #1
	b	.LBB2_3
.LBB2_1:
	mov	w8, wzr
.LBB2_2:
	tbnz	w8, #0, .LBB2_13
.LBB2_3:
	ldar	w8, [x10]
	cmp	w8, w9
	b.ne	.LBB2_12
	mov	x0, x10
	mov	w8, #98
	mov	w1, #128
	mov	x3, xzr
	//APP
	svc	#0
	//NO_APP
	cmn	x0, #4095
	b.lo	.LBB2_1
	cmn	x0, #110
	b.eq	.LBB2_10
	and	w8, w0, #0xffff
	cmp	w8, w11
	b.eq	.LBB2_9
	cmp	w8, w12
	b.ne	.LBB2_11
	mov	w8, #512
	b	.LBB2_2
.LBB2_9:
	mov	w8, #256
	b	.LBB2_2
.LBB2_10:
	mov	w8, #768
	b	.LBB2_2
.LBB2_11:
	sub	w8, w13, w0, lsl #16
	b	.LBB2_2
.LBB2_12:
	mov	w0, wzr
	ret
.LBB2_13:
	lsr	w1, w8, #16
	mov	w0, #1
	ret

== _RNvNtCsoSpHl677Cf_8o1_linux5futex4wake
	mov	x9, x8
	mov	w8, #2147483647
	mov	x3, xzr
	cmp	w1, w8
	csel	w2, w1, w8, lo
	mov	w8, #98
	mov	w1, #129
	//APP
	svc	#0
	//NO_APP
	cmn	x0, #1, lsl #12
	b.ls	.LBB3_2
	neg	w8, w0
	strh	w8, [x9, #2]
	mov	w8, #1
	strh	w8, [x9]
	ret
.LBB3_2:
	str	x0, [x9, #8]
	strh	wzr, [x9]
	ret

