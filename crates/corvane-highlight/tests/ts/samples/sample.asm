; Print a greeting and sum a small array (Linux x86-64, NASM syntax).
%define SYS_WRITE 1
%define SYS_EXIT  60
%define STDOUT    1

section .data
    greeting     db  "Hello, assembly!", 10, 0
    greeting_len equ $ - greeting - 1
    numbers      dd  3, 5, 7, 11, 0x10, 0b101
    count        equ ($ - numbers) / 4

section .bss
    total        resq 1

section .text
    global _start

; sum_array(rdi = ptr, rsi = count) -> rax
sum_array:
    xor     rax, rax
    test    rsi, rsi
    jz      .done
.loop:
    movsxd  rdx, dword [rdi]
    add     rax, rdx
    add     rdi, 4
    dec     rsi
    jnz     .loop
.done:
    ret

_start:
    mov     rax, SYS_WRITE
    mov     rdi, STDOUT
    lea     rsi, [rel greeting]
    mov     rdx, greeting_len
    syscall
    lea     rdi, [rel numbers]
    mov     rsi, count
    call    sum_array
    mov     [rel total], rax

    mov     rax, SYS_EXIT
    xor     edi, edi
    syscall
