--  Simple bounded queue with a small driver procedure.
with Ada.Text_IO;         use Ada.Text_IO;
with Ada.Integer_Text_IO;

procedure Sample is
   Max_Items : constant Positive := 8;
   type Item_Array is array (1 .. Max_Items) of Integer;

   type Queue is record
      Items : Item_Array := (others => 0);
      Count : Natural    := 0;
   end record;

   Queue_Full : exception;

   procedure Push (Q : in out Queue; Value : Integer) is
   begin
      if Q.Count >= Max_Items then
         raise Queue_Full with "queue is full";
      end if;
      Q.Count := Q.Count + 1;
      Q.Items (Q.Count) := Value;
   end Push;

   function Sum (Q : Queue) return Integer is
      Total : Integer := 0;
   begin
      for I in 1 .. Q.Count loop
         Total := Total + Q.Items (I);
      end loop;
      return Total;
   end Sum;

   Q     : Queue;
   Ready : Boolean := True;
begin
   while Ready and then Q.Count < 5 loop
      Push (Q, Q.Count * 16#10#);
   end loop;
   Put ("Sum: ");
   Ada.Integer_Text_IO.Put (Sum (Q), Width => 0);
   New_Line;
exception
   when Queue_Full => Put_Line ("overflow" & Character'Val (33));
end Sample;
