{-# OPTIONS --without-K #-}
-- Natural numbers, lists and a small proof, written in ASCII only.
module sample where

{- Everything is defined from scratch here,
   so the file needs no library imports. -}
data Nat : Set where
  zero : Nat
  suc  : Nat -> Nat

{-# BUILTIN NATURAL Nat #-}
infix 4 _==_
infixl 6 _+_
infixr 5 _::_

data _==_ {A : Set} (x : A) : A -> Set where
  refl : x == x

_+_ : Nat -> Nat -> Nat
zero  + m = m
suc n + m = suc (n + m)

cong : {A B : Set} {x y : A} (f : A -> B) -> x == y -> f x == f y
cong f refl = refl

+-zero : (n : Nat) -> n + zero == n
+-zero zero    = refl
+-zero (suc n) = cong suc (+-zero n)

data List (A : Set) : Set where
  []   : List A
  _::_ : A -> List A -> List A

length : {A : Set} -> List A -> Nat
length []        = 0
length (_ :: xs) = suc (length xs)

record Pair (A B : Set) : Set where
  constructor _,_
  field
    fst : A
    snd : B

example : length (1 :: 2 :: 3 :: []) == 3
example = refl
