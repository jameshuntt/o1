== _RNvCsbiiMJFb5GLA_16o1_codegen_probe16probe_select_u64
	movq	%rsi, %rax
	testl	%edi, %edi
	cmoveq	%rdx, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe16probe_tagged_tag
	movq	%rdi, %rax
	andl	$7, %eax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe18probe_bit_test_u64
	movl	%esi, %ecx
	cmpl	$64, %esi
	setb	%al
	shrq	%cl, %rdi
	andb	%dil, %al
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe18probe_last_set_u64
	xorl	%eax, %eax
	testq	%rdi, %rdi
	setne	%al
	lzcntq	%rdi, %rdx
	xorl	$63, %edx
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe18probe_mask_low_u64
	movl	%edi, %ecx
	movq	$-1, %rdx
	movq	$-1, %rax
	shlq	%cl, %rax
	cmpl	$64, %edi
	notq	%rax
	cmovaeq	%rdx, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe18probe_spsc_pop_u64
	movq	8(%rdi), %rcx
	cmpq	16(%rdi), %rcx
	jne	.LBB5_2
	movq	(%rdi), %rax
	movq	128(%rax), %rdx
	movq	%rdx, 16(%rdi)
	movq	8(%rdi), %rcx
	movq	$-1, %rax
	cmpq	%rdx, %rcx
	je	.LBB5_3
.LBB5_2:
	movl	%ecx, %eax
	andl	$7, %eax
	movq	(%rdi), %rdx
	movq	256(%rdx,%rax,8), %rax
	incq	%rcx
	movq	%rcx, 8(%rdi)
	movq	%rcx, (%rdx)
.LBB5_3:
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe18probe_tagged_untag
	movq	%rdi, %rcx
	andq	$-8, %rcx
	movq	%rcx, %rax
	subq	%rdi, %rax
	testq	%rcx, %rcx
	cmoveq	%rcx, %rax
	addq	%rdi, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe19probe_first_set_u64
	xorl	%eax, %eax
	testq	%rdi, %rdi
	setne	%al
	tzcntq	%rdi, %rdx
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe19probe_pop_count_u64
	popcntq	%rdi, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe19probe_spsc_push_u64
	movq	8(%rdi), %rax
	movq	%rax, %rcx
	subq	16(%rdi), %rcx
	cmpq	$8, %rcx
	jne	.LBB9_3
	movq	(%rdi), %rax
	movq	(%rax), %rcx
	movq	%rcx, 16(%rdi)
	movq	8(%rdi), %rax
	movq	%rax, %rdx
	subq	%rcx, %rdx
	cmpq	$8, %rdx
	jne	.LBB9_3
	xorl	%eax, %eax
	retq
.LBB9_3:
	movl	%eax, %ecx
	andl	$7, %ecx
	movq	(%rdi), %rdx
	movq	%rsi, 256(%rdx,%rcx,8)
	incq	%rax
	movq	%rax, 8(%rdi)
	movq	%rax, 128(%rdx)
	movb	$1, %al
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe19probe_tagged_as_ptr
	movq	%rdi, %rax
	andq	$-8, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe22probe_block_read_plain
	xorl	%eax, %eax
	cmpq	$64, %rsi
	jb	.LBB11_2
.LBB11_1:
	movzbl	(%rdi), %ecx
	addq	%rax, %rcx
	movzbl	1(%rdi), %eax
	movzbl	2(%rdi), %edx
	addq	%rax, %rdx
	addq	%rcx, %rdx
	movzbl	3(%rdi), %eax
	movzbl	4(%rdi), %ecx
	addq	%rax, %rcx
	movzbl	5(%rdi), %eax
	addq	%rcx, %rax
	addq	%rdx, %rax
	movzbl	6(%rdi), %ecx
	movzbl	7(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	8(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	9(%rdi), %edx
	addq	%rcx, %rdx
	addq	%rax, %rdx
	movzbl	10(%rdi), %eax
	movzbl	11(%rdi), %ecx
	addq	%rax, %rcx
	movzbl	12(%rdi), %eax
	addq	%rcx, %rax
	movzbl	13(%rdi), %ecx
	addq	%rax, %rcx
	movzbl	14(%rdi), %eax
	addq	%rcx, %rax
	addq	%rdx, %rax
	movzbl	15(%rdi), %ecx
	movzbl	16(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	17(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	18(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	19(%rdi), %r8d
	addq	%rdx, %r8
	movzbl	20(%rdi), %ecx
	addq	%r8, %rcx
	addq	%rax, %rcx
	movzbl	21(%rdi), %eax
	movzbl	22(%rdi), %edx
	addq	%rax, %rdx
	movzbl	23(%rdi), %eax
	addq	%rdx, %rax
	movzbl	24(%rdi), %edx
	addq	%rax, %rdx
	movzbl	25(%rdi), %eax
	addq	%rdx, %rax
	movzbl	26(%rdi), %edx
	addq	%rax, %rdx
	movzbl	27(%rdi), %eax
	addq	%rdx, %rax
	addq	%rcx, %rax
	movzbl	28(%rdi), %ecx
	movzbl	29(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	30(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	31(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	32(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	33(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	34(%rdi), %r8d
	addq	%rdx, %r8
	movzbl	35(%rdi), %ecx
	addq	%r8, %rcx
	addq	%rax, %rcx
	movzbl	36(%rdi), %eax
	movzbl	37(%rdi), %edx
	addq	%rax, %rdx
	movzbl	38(%rdi), %eax
	addq	%rdx, %rax
	movzbl	39(%rdi), %edx
	addq	%rax, %rdx
	movzbl	40(%rdi), %eax
	addq	%rdx, %rax
	movzbl	41(%rdi), %edx
	addq	%rax, %rdx
	movzbl	42(%rdi), %eax
	addq	%rdx, %rax
	movzbl	43(%rdi), %edx
	addq	%rax, %rdx
	movzbl	44(%rdi), %eax
	addq	%rdx, %rax
	addq	%rcx, %rax
	movzbl	45(%rdi), %ecx
	movzbl	46(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	47(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	48(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	49(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	50(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	51(%rdi), %ecx
	addq	%rdx, %rcx
	movzbl	52(%rdi), %edx
	addq	%rcx, %rdx
	movzbl	53(%rdi), %r8d
	addq	%rdx, %r8
	movzbl	54(%rdi), %ecx
	addq	%r8, %rcx
	addq	%rax, %rcx
	movzbl	55(%rdi), %eax
	movzbl	56(%rdi), %edx
	addq	%rax, %rdx
	movzbl	57(%rdi), %eax
	addq	%rdx, %rax
	movzbl	58(%rdi), %edx
	addq	%rax, %rdx
	movzbl	59(%rdi), %eax
	addq	%rdx, %rax
	movzbl	60(%rdi), %edx
	addq	%rax, %rdx
	movzbl	61(%rdi), %eax
	addq	%rdx, %rax
	movzbl	62(%rdi), %edx
	addq	%rax, %rdx
	movzbl	63(%rdi), %eax
	addq	%rdx, %rax
	addq	%rcx, %rax
	addq	$-64, %rsi
	addq	$64, %rdi
	cmpq	$63, %rsi
	ja	.LBB11_1
.LBB11_2:
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe24probe_isolate_lowest_u64
	blsiq	%rdi, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe27probe_block_transform_plain
	movq	$-1, %rax
	cmpq	%rcx, %rsi
	jne	.LBB13_5
	movq	%rsi, %rax
	cmpq	$64, %rsi
	jb	.LBB13_4
	xorl	%ecx, %ecx
	movaps	.LCPI13_0(%rip), %xmm0
	movq	%rsi, %rax
.LBB13_3:
	addq	$-64, %rax
	movups	(%rdi,%rcx), %xmm1
	movups	16(%rdi,%rcx), %xmm2
	xorps	%xmm0, %xmm1
	xorps	%xmm0, %xmm2
	movups	%xmm1, (%rdx,%rcx)
	movups	%xmm2, 16(%rdx,%rcx)
	movups	32(%rdi,%rcx), %xmm1
	movups	48(%rdi,%rcx), %xmm2
	xorps	%xmm0, %xmm1
	xorps	%xmm0, %xmm2
	movups	%xmm1, 32(%rdx,%rcx)
	movups	%xmm2, 48(%rdx,%rcx)
	addq	$64, %rcx
	cmpq	$63, %rax
	ja	.LBB13_3
.LBB13_4:
	subq	%rax, %rsi
	shrq	$6, %rsi
	movq	%rsi, %rax
.LBB13_5:
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe28probe_leading_zero_count_u64
	lzcntq	%rdi, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe29probe_trailing_zero_count_u32
	tzcntl	%edi, %eax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe29probe_trailing_zero_count_u64
	tzcntq	%rdi, %rax
	retq

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe28probe_block_transform_policy (identical code; alias of _RNvCsbiiMJFb5GLA_16o1_codegen_probe27probe_block_transform_plain)

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe23probe_select_masked_u64 (identical code; alias of _RNvCsbiiMJFb5GLA_16o1_codegen_probe16probe_select_u64)

== _RNvCsbiiMJFb5GLA_16o1_codegen_probe23probe_block_read_policy (identical code; alias of _RNvCsbiiMJFb5GLA_16o1_codegen_probe22probe_block_read_plain)

