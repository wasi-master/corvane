;;; lib.scm — utility library, ünïcödé comment
;; SPDX-License-Identifier: MIT

#| Block comment
   spanning | several lines
   with a pipe inside |#

(define-module (corvane util)
  #:export (fold-left compose string-join* λ-test))

(import (rnrs base (6))
        (only (srfi :1) iota filter))

(define (square x) (* x x))
(define pi 3.14159)
(define big 1e10)
(define tiny -2.5e-3)
(define ratio 22/7)
(define neg-ratio -1/3)
(define cplx 1+2i)
(define polar 1@2)
(define imag +i)
(define inexact 12#.#)
(define hash-num 12##)
(define dot .5)

(define-syntax swap!
  (syntax-rules ()
    [(_ a b) (let ((tmp a)) (set! a b) (set! b tmp))]))

(define (fold-left f init lst)
  (if (null? lst)
      init
      (fold-left f (f init (car lst)) (cdr lst))))

(define compose
  (lambda fs
    (if (null? fs)
        (lambda (x) x)
        (lambda (x) ((car fs) ((apply compose (cdr fs)) x))))))

(let loop ((i 0) (acc '()))
  (when (< i 10)
    (loop (+ i 1) (cons i acc))))

(let* ([a 1] [b (+ a 1)])
  (display (list a b))
  (newline))

(define radixes
  (list #b1010 #B-101 #o755 #O17 #xFF #xdead/beef #d99 #D-42
        #e1.5 #i3/4 #x#e1F #e#x10 #b#i101 #i#d12 #e #z #b102 #xZZ))

(define booleans (list #t #f #true #false #T #F))
(define chars (list #\a #\space #\newline #\λ #\())

(define str "a simple string")
(define esc "escaped \"quotes\" and \\ backslash")
(define multi "first line
second line
third line")
(define sym '|weird symbol with spaces|)
(define unicode "naïve café 日本語 🎉")

(define quoted '(1 2 (3 4) "five" six))
(define quoted-vec '[a b c])
(define q 'symbol)
(define qq `(1 ,(+ 1 1) ,@(list 3 4)))

#;(this whole form (is commented out))
#; single-token-comment (display "visible")
#;[bracketed (comment)]

(define (string-join* strs sep)
  (cond
    ((null? strs) "")
    ((null? (cdr strs)) (car strs))
    (else (string-append (car strs) sep (string-join* (cdr strs) sep)))))

(call-with-current-continuation
  (lambda (k)
    (for-each (lambda (x) (if (negative? x) (k x))) '(54 0 37 -3 245 19))
    #t))

(λ (x) (* x 2))
(define λ-test (λ (y) y))
(vector-ref #(1 2 3) 0)
(string->symbol "abc")
(char->integer #\A)
(exact->inexact 1/3)
-foo +bar .baz 1abc 1.2.3 +5 -
(mismatched ]
[also mismatched )
)))
; trailing comment
#| unterminated block comment
still commented
(define unterminated "string never closed