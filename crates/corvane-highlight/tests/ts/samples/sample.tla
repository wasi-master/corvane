------------------------------ MODULE sample ------------------------------
(***************************************************************************)
(* A bounded buffer shared by a producer and a consumer.                   *)
(***************************************************************************)
EXTENDS Naturals, Sequences, TLC

CONSTANTS Capacity, Items
ASSUME Capacity \in Nat \ {0}

VARIABLES buffer, produced, consumed
vars == <<buffer, produced, consumed>>

TypeOK == /\ buffer \in Seq(Items)
          /\ produced \in Nat /\ consumed \in Nat

Init == buffer = <<>> /\ produced = 0 /\ consumed = 0

\* The producer appends any item while there is room.
Produce ==
    /\ Len(buffer) < Capacity
    /\ \E i \in Items : buffer' = Append(buffer, i)
    /\ produced' = produced + 1
    /\ UNCHANGED consumed

Consume ==
    /\ buffer # <<>>
    /\ buffer' = Tail(buffer)
    /\ consumed' = consumed + 1
    /\ UNCHANGED produced

Next == Produce \/ Consume
Spec == Init /\ [][Next]_vars /\ WF_vars(Consume)

Bounded == Len(buffer) <= Capacity
Progress == \A n \in 1..3 : (produced >= n) ~> (consumed >= n)
Status == LET size == Len(buffer)
          IN CASE size = 0 -> "empty" [] size = Capacity -> "full" [] OTHER -> "partial"
Squares == [k \in 1..5 |-> k * k]
Oldest == IF buffer = <<>> THEN CHOOSE i \in Items : TRUE ELSE Head(buffer)

THEOREM Spec => []TypeOK
=============================================================================
