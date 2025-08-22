.global _trampoline

.section .text

_trampoline:
    # Save callee-saved registers
    pushq %rbp

    // Stack is now 16 byte aligned
    movq %rsp, %rbp

    pushq %rsp
    pushq %rbx
    pushq %r12
    pushq %r13
    pushq %r14
    pushq %r15

    pushq $_main_data

    // We have pushed 7 registers and 1 pointer so the stack is aligned to 16 bytes - 8
    // We don't push %rbp from inside of main so when we push %rip during call we will be
    // correctly aligned
    call _main

    // Pop _main_data
    addq $8, %rsp

    popq %r15
    popq %r14
    popq %r13
    popq %r12
    popq %rbx
    popq %rsp
    popq %rbp

    ret

