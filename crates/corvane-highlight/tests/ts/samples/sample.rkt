#lang racket/base
;; A tiny bank account simulation with structs, contracts and match.

(require racket/contract
         racket/match
         (only-in racket/list first rest empty?))

(provide (contract-out
          [struct account ([owner string?] [balance exact-nonnegative-integer?])]
          [apply-tx (-> account? tx? account?)]))

#| Transactions are deposits, withdrawals or fees.
   Amounts are in cents. |#
(struct account (owner balance) #:transparent)
(struct tx (kind amount) #:prefab)

(define fee-rate 1/100)
(define max-withdrawal #e1e6)

(define (apply-tx acct t)
  (match t
    [(tx 'deposit n) (struct-copy account acct [balance (+ (account-balance acct) n)])]
    [(tx 'withdraw n)
     #:when (<= n (min max-withdrawal (account-balance acct)))
     (struct-copy account acct [balance (- (account-balance acct) n)])]
    [(tx 'fee _)
     (define fee (ceiling (* fee-rate (account-balance acct))))
     (struct-copy account acct [balance (- (account-balance acct) fee)])]
    [_ (raise-argument-error 'apply-tx "valid transaction" t)]))

(define (run acct txs)
  (for/fold ([a acct]) ([t (in-list txs)])
    (apply-tx a t)))

(module+ main
  (define start (account "ada" 10000))
  (define txs (list (tx 'deposit 2500) (tx 'withdraw 400) (tx 'fee 0)))
  (define final (run start txs))
  (printf "~a has ~a cents\n" (account-owner final) (account-balance final))
  (let loop ([i 0] [chars '(#\a #\space #\z)])
    (unless (empty? chars)
      (displayln (list i (first chars) #t 3.5))
      (loop (add1 i) (rest chars)))))
