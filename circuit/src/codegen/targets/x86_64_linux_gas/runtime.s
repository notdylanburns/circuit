.set SYS_WRITE, 1
.set SYS_RT_SIGACTION, 13
.set SYS_RT_SIGRETURN, 15
.set SYS_NANOSLEEP, 35
.set SYS_EXIT, 60
.set SIZEOF_SIGSET_T, 8

.section .text
.global _start

_start:
    call setup_signals

    lea _MAIN_RESV(%rip), %rax
    pushq %rax
.loop:
    call _MAIN_CODE
    call sleep
    jmp .loop

exit:
    movq $SYS_EXIT, %rax
    syscall

sleep:
    movq $SYS_NANOSLEEP, %rax
    movq $sleep_timespec, %rdi
    xor %rsi, %rsi  # don't care about remaining time
    syscall

    ret

setup_signals:
    movq $SYS_RT_SIGACTION, %rax   # sigaction
    movq $2, %rdi    # SIGINT
    movq $sigint_handler, .sa_handler

    leaq signal_action(%rip), %rsi  # pointer to sigaction struct
    xor %rdx, %rdx  # don't care about old action
    mov $SIZEOF_SIGSET_T, %r10

    syscall

    test %rax, %rax
    js .sigaction_failed

    ret

.sigaction_failed:
    push %rax
    mov $SYS_WRITE, %rax
    mov $2, %rdi  # stderr

    lea sigaction_failed_msg(%rip), %rsi  # pointer to message
    mov $len_sigaction_failed_msg, %rdx  # message length
    syscall

    pop %rdi
    call exit

sigint_handler:
    mov $SYS_WRITE, %rax
    mov $2, %rdi  # stderr

    lea sigint_msg(%rip), %rsi  # pointer to message
    mov $len_sigint_msg, %rdx  # message length
    syscall

    xor %rdi, %rdi  # exit code 0
    call exit


signal_restorer:
    mov $SYS_RT_SIGRETURN, %rax
    syscall


.section .rodata
sigint_msg: .asciz "SIGINT received, exiting...\n"
len_sigint_msg = . - sigint_msg
sigaction_failed_msg: .asciz "failed to initialise signal handlers\n"
len_sigaction_failed_msg = . - sigaction_failed_msg

sleep_timespec:
    .quad 0  # seconds
    .quad 100000000  # nanoseconds (0.1 seconds)

.section .data
signal_action:
    .sa_handler:    .quad 0
    .sa_flags:      .quad 0x04000000    # SA_RESTORER
    .sa_restorer:   .quad signal_restorer
    .sa_mask:       .zero SIZEOF_SIGSET_T  # zero out the mask
