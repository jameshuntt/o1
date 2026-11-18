== _RNvCsiv4hAGqeh2l_16o1_codegen_probe16probe_select_u64
	cmp	w0, #0
	csel	x0, x1, x2, ne
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe16probe_tagged_tag
	and	x0, x0, #0x7
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe18probe_bit_test_u64
	cmp	w1, #64
	lsr	x8, x0, x1
	cset	w9, lo
	and	w0, w9, w8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe18probe_last_set_u64
	cmp	x0, #0
	clz	x9, x0
	cset	w8, ne
	eor	w1, w9, #0x3f
	mov	w0, w8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe18probe_mask_low_u64
	mov	x8, #-1
	cmp	w0, #64
	lsl	x9, x8, x0
	csinv	x0, x8, x9, hs
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe18probe_spsc_pop_u64
	ldp	x8, x9, [x0, #8]
	cmp	x8, x9
	b.ne	.LBB5_3
	ldr	x8, [x0]
	add	x8, x8, #128
	ldar	x9, [x8]
	ldr	x8, [x0, #8]
	cmp	x8, x9
	str	x9, [x0, #16]
	b.ne	.LBB5_3
	mov	x0, #-1
	ret
.LBB5_3:
	ldr	x9, [x0]
	and	x10, x8, #0x7
	add	x11, x8, #1
	str	x11, [x0, #8]
	add	x10, x9, x10, lsl #3
	ldr	x8, [x10, #256]
	stlr	x11, [x9]
	mov	x0, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe18probe_tagged_untag
	ands	x8, x0, #0xfffffffffffffff8
	sub	x8, x8, x0
	tst	x0, #0xfffffffffffffff8
	csel	x8, xzr, x8, eq
	add	x0, x0, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe19probe_first_set_u64
	rbit	x8, x0
	cmp	x0, #0
	cset	w0, ne
	clz	x1, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe19probe_pop_count_u64
	fmov	d0, x0
	cnt	v0.8b, v0.8b
	addv	b0, v0.8b
	fmov	w0, s0
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe19probe_spsc_push_u64
	ldp	x8, x9, [x0, #8]
	sub	x9, x8, x9
	cmp	x9, #8
	b.ne	.LBB9_3
	ldp	x10, x8, [x0]
	ldar	x9, [x10]
	sub	x10, x8, x9
	str	x9, [x0, #16]
	cmp	x10, #8
	b.ne	.LBB9_3
	mov	w0, wzr
	ret
.LBB9_3:
	ldr	x9, [x0]
	and	x10, x8, #0x7
	add	x8, x8, #1
	str	x8, [x0, #8]
	mov	w0, #1
	add	x10, x9, x10, lsl #3
	add	x9, x9, #128
	str	x1, [x10, #256]
	stlr	x8, [x9]
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe19probe_tagged_as_ptr
	and	x0, x0, #0xfffffffffffffff8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe22probe_block_read_plain
	cmp	x1, #64
	b.lo	.LBB11_4
	mov	x8, xzr
.LBB11_2:
	ldp	q2, q0, [x0, #16]
	sub	x1, x1, #64
	ldr	q1, [x0]
	ldr	q3, [x0, #48]
	cmp	x1, #63
	add	x0, x0, #64
	ushll	v4.8h, v0.8b, #0
	ushll	v5.8h, v1.8b, #0
	ushll	v6.8h, v3.8b, #0
	ushll	v7.8h, v2.8b, #0
	ushll2	v0.8h, v0.16b, #0
	ushll2	v3.8h, v3.16b, #0
	ushll2	v2.8h, v2.16b, #0
	ushll2	v1.8h, v1.16b, #0
	ushll	v16.4s, v4.4h, #0
	ushll	v17.4s, v5.4h, #0
	ushll	v18.4s, v6.4h, #0
	ushll2	v4.4s, v4.8h, #0
	ushll2	v5.4s, v5.8h, #0
	ushll2	v6.4s, v6.8h, #0
	ushll2	v19.4s, v3.8h, #0
	ushll2	v20.4s, v2.8h, #0
	ushll2	v21.4s, v7.8h, #0
	ushll2	v22.4s, v0.8h, #0
	ushll2	v23.4s, v1.8h, #0
	ushll	v7.4s, v7.4h, #0
	ushll	v3.4s, v3.4h, #0
	ushll	v2.4s, v2.4h, #0
	ushll	v0.4s, v0.4h, #0
	ushll	v1.4s, v1.4h, #0
	uaddl2	v24.2d, v20.4s, v19.4s
	uaddl2	v26.2d, v21.4s, v6.4s
	uaddl2	v25.2d, v23.4s, v22.4s
	uaddl2	v27.2d, v5.4s, v4.4s
	uaddl	v19.2d, v20.2s, v19.2s
	uaddl2	v20.2d, v2.4s, v3.4s
	uaddl	v22.2d, v23.2s, v22.2s
	uaddl	v6.2d, v21.2s, v6.2s
	uaddl2	v23.2d, v1.4s, v0.4s
	uaddl2	v21.2d, v7.4s, v18.4s
	uaddl	v4.2d, v5.2s, v4.2s
	uaddl2	v5.2d, v17.4s, v16.4s
	uaddl	v2.2d, v2.2s, v3.2s
	uaddl	v0.2d, v1.2s, v0.2s
	uaddl	v1.2d, v7.2s, v18.2s
	uaddl	v3.2d, v17.2s, v16.2s
	add	v7.2d, v25.2d, v24.2d
	add	v16.2d, v27.2d, v26.2d
	add	v17.2d, v23.2d, v20.2d
	add	v18.2d, v22.2d, v19.2d
	add	v5.2d, v5.2d, v21.2d
	add	v4.2d, v4.2d, v6.2d
	add	v0.2d, v0.2d, v2.2d
	add	v1.2d, v3.2d, v1.2d
	add	v2.2d, v16.2d, v7.2d
	add	v3.2d, v5.2d, v17.2d
	add	v4.2d, v4.2d, v18.2d
	add	v0.2d, v1.2d, v0.2d
	add	v1.2d, v3.2d, v2.2d
	add	v0.2d, v0.2d, v4.2d
	add	v0.2d, v0.2d, v1.2d
	addp	d0, v0.2d
	fmov	x9, d0
	add	x8, x9, x8
	b.hi	.LBB11_2
	mov	x0, x8
	ret
.LBB11_4:
	mov	x8, xzr
	mov	x0, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe24probe_isolate_lowest_u64
	neg	x8, x0
	and	x0, x0, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe27probe_block_transform_plain
	cmp	x1, x3
	b.ne	.LBB13_5
	cmp	x1, #64
	mov	x8, x1
	b.lo	.LBB13_4
	movi	v0.16b, #90
	mov	x8, x1
.LBB13_3:
	ldp	q1, q2, [x0]
	sub	x8, x8, #64
	ldp	q3, q4, [x0, #32]
	cmp	x8, #63
	add	x0, x0, #64
	eor	v1.16b, v1.16b, v0.16b
	eor	v2.16b, v2.16b, v0.16b
	eor	v3.16b, v3.16b, v0.16b
	eor	v4.16b, v4.16b, v0.16b
	stp	q1, q2, [x2]
	stp	q3, q4, [x2, #32]
	add	x2, x2, #64
	b.hi	.LBB13_3
.LBB13_4:
	sub	x8, x1, x8
	lsr	x0, x8, #6
	ret
.LBB13_5:
	mov	x0, #-1
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe28probe_leading_zero_count_u64
	clz	x0, x0
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe29probe_trailing_zero_count_u32
	rbit	w8, w0
	clz	w0, w8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe29probe_trailing_zero_count_u64
	rbit	x8, x0
	clz	x0, x8
	ret

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe23probe_select_masked_u64 (identical code; alias of _RNvCsiv4hAGqeh2l_16o1_codegen_probe16probe_select_u64)

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe23probe_block_read_policy (identical code; alias of _RNvCsiv4hAGqeh2l_16o1_codegen_probe22probe_block_read_plain)

== _RNvCsiv4hAGqeh2l_16o1_codegen_probe28probe_block_transform_policy (identical code; alias of _RNvCsiv4hAGqeh2l_16o1_codegen_probe27probe_block_transform_plain)

