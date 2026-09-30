; ModuleID = 'sample.c'
source_filename = "sample.c"
target triple = "arm64-apple-macosx14.0.0"

%struct.Point = type { i32, i32 }
@.str = private unnamed_addr constant [10 x i8] c"sum = %d\0A\00", align 1
@counter = global i32 0, align 4
declare i32 @printf(ptr noundef, ...)

; Returns 0 + 1 + ... + (n - 1).
define i32 @sum_to(i32 %n) #0 {
entry:
  %cmp = icmp sgt i32 %n, 0
  br i1 %cmp, label %loop, label %exit

loop:
  %i = phi i32 [ 0, %entry ], [ %next, %loop ]
  %acc = phi i32 [ 0, %entry ], [ %acc.next, %loop ]
  %acc.next = add nsw i32 %acc, %i
  %next = add nuw nsw i32 %i, 1
  %done = icmp eq i32 %next, %n
  br i1 %done, label %exit, label %loop

exit:
  %result = phi i32 [ 0, %entry ], [ %acc.next, %loop ]
  ret i32 %result
}

define i32 @main() {
  %p = alloca %struct.Point, align 4
  %x = getelementptr inbounds %struct.Point, ptr %p, i32 0, i32 0
  store i32 10, ptr %x, align 4
  %v = load i32, ptr %x, align 4
  %s = call i32 @sum_to(i32 %v)
  %old = atomicrmw add ptr @counter, i32 1 seq_cst
  %f = sitofp i32 %s to double
  %big = fcmp ogt double %f, 1.0e+01
  %sel = select i1 %big, i32 %s, i32 -1
  %r = call i32 (ptr, ...) @printf(ptr @.str, i32 %sel)
  ret i32 0
}

attributes #0 = { nounwind "frame-pointer"="non-leaf" }
!llvm.ident = !{!0}
!0 = !{!"hand-written sample"}
