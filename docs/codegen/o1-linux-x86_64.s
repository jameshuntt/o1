== _RNvNtCs6kgf3TYBrmy_8o1_linux3uds10recv_frame
	pushq	%rbp
	pushq	%r15
	pushq	%r14
	pushq	%r13
	pushq	%r12
	pushq	%rbx
	subq	$40, %rsp
	movq	%rdx, %r15
	movq	%rdi, 8(%rsp)
	movl	%esi, %r12d
	movl	$4, %ebp
	leaq	.LJTI0_0(%rip), %rbx
	jmp	.LBB0_1
.LBB0_16:
	cmpq	$-4, %rax
	jne	.LBB0_17
.LBB0_1:
	movl	(%r15), %eax
	movl	%eax, %edx
	subl	$4, %edx
	cmovbl	%ebp, %edx
	movslq	(%rbx,%rdx,4), %rcx
	addq	%rbx, %rcx
	jmpq	*%rcx
.LBB0_6:
	movzbl	4(%r15), %r14d
	cmpq	$4, %r14
	ja	.LBB0_8
	movl	$4, %r13d
	subq	%r14, %r13
	leaq	56(%r15), %rax
	addq	%rax, %r14
	jmp	.LBB0_13
.LBB0_12:
	movq	32(%r15), %rax
	movq	16(%r15), %r13
	movq	24(%r15), %r14
	cmpq	%r13, %rax
	cmovbq	%rax, %r13
	jmp	.LBB0_13
.LBB0_2:
	movq	8(%r15), %rsi
	movq	16(%r15), %r14
	movq	32(%r15), %rdx
	movq	%rsi, %r13
	subq	%r14, %r13
	jb	.LBB0_9
	cmpq	%rdx, %rsi
	ja	.LBB0_9
	addq	24(%r15), %r14
.LBB0_13:
	xorl	%eax, %eax
	movq	%r12, %rdi
	movq	%r14, %rsi
	movq	%r13, %rdx
	#APP

	syscall

	#NO_APP
	cmpq	$-4096, %rax
	ja	.LBB0_16
	leaq	16(%rsp), %rdi
	movq	%r15, %rsi
	movq	%rax, %rdx
	callq	*_RNvMs0_NtCsiYFcV3EeMWD_7o1_core5frameNtB5_12FrameDecoder6filled@GOTPCREL(%rip)
	movl	16(%rsp), %eax
	cmpl	$-1, %eax
	jne	.LBB0_15
	movq	24(%rsp), %rcx
	testq	%rcx, %rcx
	je	.LBB0_1
	movq	32(%rsp), %rax
	cmpl	$1, %ecx
	je	.LBB0_28
	cmpl	$2, %ecx
	movq	8(%rsp), %rdx
	jne	.LBB0_24
	movq	$1, 8(%rdx)
	movq	%rax, 16(%rdx)
	jmp	.LBB0_26
.LBB0_17:
	movzwl	%ax, %ecx
	cmpl	$65525, %ecx
	jne	.LBB0_18
	movq	8(%rsp), %rdx
	movq	$3, 8(%rdx)
	jmp	.LBB0_26
.LBB0_15:
	movl	20(%rsp), %ecx
	movups	24(%rsp), %xmm0
	movq	8(%rsp), %rdx
	movw	$1, (%rdx)
	movl	%eax, 8(%rdx)
	movl	%ecx, 12(%rdx)
	movups	%xmm0, 16(%rdx)
	jmp	.LBB0_27
.LBB0_18:
	negl	%eax
	movq	8(%rsp), %rdx
	movw	$0, (%rdx)
	movw	%ax, 2(%rdx)
	jmp	.LBB0_27
.LBB0_28:
	movq	8(%rsp), %rdx
	movq	$0, 8(%rdx)
	movq	%rax, 16(%rdx)
	jmp	.LBB0_26
.LBB0_24:
	movq	$2, 8(%rdx)
.LBB0_26:
	movw	$-1, (%rdx)
	jmp	.LBB0_27
.LBB0_10:
	movl	%edx, %eax
	jmp	.LBB0_11
.LBB0_5:
	movl	4(%r15), %ecx
	movq	8(%r15), %r14
	movq	16(%r15), %r13
.LBB0_11:
	movq	8(%rsp), %rdx
	movw	$1, (%rdx)
	movl	%eax, 8(%rdx)
	movl	%ecx, 12(%rdx)
	movq	%r14, 16(%rdx)
	movq	%r13, 24(%rdx)
.LBB0_27:
	movq	%rdx, %rax
	addq	$40, %rsp
	popq	%rbx
	popq	%r12
	popq	%r13
	popq	%r14
	popq	%r15
	popq	%rbp
	retq
.LBB0_9:
	leaq	.Lanon.2453359d43cd01ee5cef5ff5132c261f.2(%rip), %rcx
	movq	%r14, %rdi
	callq	*_RNvNtNtCs4NRVxsYgnAr_4core5slice5index16slice_index_fail@GOTPCREL(%rip)
.LBB0_8:
	leaq	.Lanon.2453359d43cd01ee5cef5ff5132c261f.1(%rip), %rcx
	movl	$4, %esi
	movl	$4, %edx
	movq	%r14, %rdi
	callq	*_RNvNtNtCs4NRVxsYgnAr_4core5slice5index16slice_index_fail@GOTPCREL(%rip)

== _RNvNtCs6kgf3TYBrmy_8o1_linux3uds10send_frame
	pushq	%r15
	pushq	%r14
	pushq	%rbx
	subq	$96, %rsp
	movq	%rdx, %r9
	movq	%rdi, %r8
	movq	8(%rdx), %rdx
	movq	16(%r9), %r10
	leaq	4(%rdx), %rax
	cmpq	%r10, %rax
	jne	.LBB1_1
.LBB1_16:
	movb	$0, 2(%r8)
.LBB1_17:
	movw	$-1, (%r8)
.LBB1_18:
	movq	%r8, %rax
	addq	$96, %rsp
	popq	%rbx
	popq	%r14
	popq	%r15
	retq
.LBB1_1:
	leaq	24(%r9), %rbx
	leaq	72(%rsp), %r14
	movl	%esi, %edi
	leaq	8(%rsp), %r15
	xorps	%xmm0, %xmm0
	leaq	40(%rsp), %rsi
	jmp	.LBB1_2
.LBB1_15:
	cmpq	%r10, %rcx
	je	.LBB1_16
.LBB1_2:
	cmpq	$4, %r10
	jae	.LBB1_3
	movl	$4, %ecx
	subq	%r10, %rcx
	addq	%rbx, %r10
	movq	(%r9), %rax
	jmp	.LBB1_7
.LBB1_3:
	addq	$-4, %r10
	movq	%rdx, %r11
	subq	%r10, %r11
	jb	.LBB1_5
	movq	%r10, %rax
	addq	(%r9), %rax
	movl	$1, %r10d
	xorl	%ecx, %ecx
	movq	%r11, %rdx
.LBB1_7:
	movq	%r10, 8(%rsp)
	movq	%rcx, 16(%rsp)
	movq	%rax, 24(%rsp)
	movq	%rdx, 32(%rsp)
	movq	$0, 40(%rsp)
	movl	$0, 48(%rsp)
	movq	%r15, 56(%rsp)
	movq	$2, 64(%rsp)
	movups	%xmm0, (%r14)
	movl	$0, 16(%r14)
	movl	$46, %eax
	movl	$16384, %edx
	#APP

	syscall

	#NO_APP
	cmpq	$-4096, %rax
	jbe	.LBB1_8
	cmpq	$-4, %rax
	jne	.LBB1_11
	movq	8(%r9), %rdx
	movq	16(%r9), %r10
	leaq	4(%rdx), %rcx
	jmp	.LBB1_15
.LBB1_8:
	testq	%rax, %rax
	je	.LBB1_9
	movq	8(%r9), %rdx
	movq	16(%r9), %r10
	leaq	4(%rdx), %rcx
	movq	%rcx, %r11
	subq	%r10, %r11
	cmpq	%r11, %rax
	ja	.LBB1_19
	addq	%rax, %r10
	movq	%r10, 16(%r9)
	jmp	.LBB1_15
.LBB1_11:
	movzwl	%ax, %ecx
	cmpl	$65525, %ecx
	jne	.LBB1_12
	movb	$1, 2(%r8)
	jmp	.LBB1_17
.LBB1_9:
	movw	$3, (%r8)
	jmp	.LBB1_18
.LBB1_12:
	negl	%eax
	movw	$0, (%r8)
	movw	%ax, 2(%r8)
	jmp	.LBB1_18
.LBB1_19:
	movw	$2, (%r8)
	movq	$1, 8(%r8)
	movq	%rax, 16(%r8)
	movq	%r11, 24(%r8)
	jmp	.LBB1_18
.LBB1_5:
	leaq	.Lanon.2453359d43cd01ee5cef5ff5132c261f.3(%rip), %rcx
	movq	%r10, %rdi
	movq	%rdx, %rsi
	callq	*_RNvNtNtCs4NRVxsYgnAr_4core5slice5index16slice_index_fail@GOTPCREL(%rip)

== _RNvNtCs6kgf3TYBrmy_8o1_linux5futex16wait_while_equal
	movl	%esi, %r8d
	movl	%esi, %edx
	movl	$128, %esi
	jmp	.LBB2_1
.LBB2_6:
	movl	$256, %r9d
.LBB2_11:
	testb	$1, %r9b
	jne	.LBB2_12
.LBB2_1:
	movl	(%rdi), %eax
	xorl	%r9d, %r9d
	cmpl	%r8d, %eax
	jne	.LBB2_2
	movl	$202, %eax
	xorl	%r10d, %r10d
	#APP

	syscall

	#NO_APP
	cmpq	$-4095, %rax
	jb	.LBB2_11
	cmpq	$-110, %rax
	je	.LBB2_10
	movzwl	%ax, %ecx
	cmpl	$65525, %ecx
	je	.LBB2_6
	cmpl	$65532, %ecx
	jne	.LBB2_8
	movl	$512, %r9d
	jmp	.LBB2_11
.LBB2_10:
	movl	$768, %r9d
	jmp	.LBB2_11
.LBB2_8:
	negl	%eax
	shll	$16, %eax
	incl	%eax
	movl	%eax, %r9d
	jmp	.LBB2_11
.LBB2_2:
	movl	%r9d, %eax
	retq
.LBB2_12:
	shrl	$16, %r9d
	movl	%r9d, %edx
	movw	$1, %r9w
	movl	%r9d, %eax
	retq

== _RNvNtCs6kgf3TYBrmy_8o1_linux5futex4wake
	movq	%rsi, %rcx
	movq	%rdi, %r8
	cmpl	$2147483647, %edx
	movl	$2147483647, %eax
	cmovael	%eax, %edx
	movl	$202, %eax
	movl	$129, %esi
	movq	%rcx, %rdi
	xorl	%r10d, %r10d
	#APP

	syscall

	#NO_APP
	cmpq	$-4096, %rax
	jbe	.LBB3_1
	negl	%eax
	movw	%ax, 2(%r8)
	movw	$1, %ax
	movw	%ax, (%r8)
	movq	%r8, %rax
	retq
.LBB3_1:
	movq	%rax, 8(%r8)
	xorl	%eax, %eax
	movw	%ax, (%r8)
	movq	%r8, %rax
	retq

