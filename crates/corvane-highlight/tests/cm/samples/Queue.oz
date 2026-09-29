% Oz sample — une file d'attente fonctionnelle
/* block comment
   spanning lines * with / inside
   done */
functor
import
   System Application
   Browser(browse:Browse)
export
   newQueue:NewQueue
define
   fun {NewQueue}
      q(front:nil back:nil)
   end

   proc {Push Q X ?R}
      R = {AdjoinAt Q back X|Q.back}
   end

   fun {`weird name` A B} A + B end
   proc {$ X} {System.showInfo X} end

   class Counter from BaseObject
      attr count:0 step
      feat name:'counter'
      meth init(Step)
         count := 0
         step := Step
      end
      meth inc
         count := @count + @step
      end
      meth `quoted meth` skip end
   end

   local X Y Z in
      X = 42
      Y = ~17
      Z = 3.14e~2 + 0xFF - ~0x1a * 2.5E+3
      if X >= Y andthen Y =< Z orelse X \= 0 then
         {Browse X#Y#Z}
      elseif X == Y then skip
      else
         raise myError(X) end
      end
      case [1 2 3] of H|T then {Browse H}
      [] nil then skip
      else skip end
      for I in 1..10 do {System.show I} end
      thread {Delay 100} end
      try {Foo} catch E then {Browse E} finally skip end
      lock L then skip end
      Cell = {NewCell unit}
      Cell := true
      B = X mod 3 div 2
      S = "a \"string\" with ünïcödé ✓"
      A = 'atom with \' quote'
      C = &a
      D = x(1:a 2:b)...
      E <- F
      G :: H ::: I
      J = X.1 + @count
      K = !!L
      M = ~ N
      trueish = falsey
      nilé = unité
   end
   declare Q2 = {NewQueue}
   class
   meth
   fun
end
"multi-line string \
continues here"
'unterminated
S2 = "open at end of file
