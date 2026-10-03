! solver.f90 — conjugate-gradient solver for a sparse SPD system
! Ünïcode in a comment: 日本語 ✓ 😀
module sparse_solver
  use, intrinsic :: iso_c_binding, only: c_int, c_double, c_ptr, c_null_ptr
  use iso_fortran_env, only: real64, int32, output_unit
  implicit none
  private
  public :: csr_matrix, cg_solve, matvec

  integer, parameter :: dp = real64
  real(kind=dp), parameter :: TOL = 1.0e-10_dp, PI = 3.14159265358979d0
  integer, parameter :: MAX_ITER = 10000
  character(len=*), parameter :: VERSION = "2.1.0"
  character(len=*), parameter :: QUOTED = 'it''s "fine"'
  logical, save :: verbose = .false.

  type :: csr_matrix
    integer :: n = 0, nnz = 0
    integer, allocatable :: row_ptr(:), col_idx(:)
    real(dp), allocatable :: val(:)
  contains
    procedure, pass :: apply => matvec
    final :: destroy
  end type csr_matrix

  interface norm
    module procedure norm2_dp
  end interface norm

contains

  pure function norm2_dp(x) result(r)
    real(dp), intent(in) :: x(:)
    real(dp) :: r
    r = sqrt(dot_product(x, x))
  end function norm2_dp

  subroutine matvec(a, x, y)
    class(csr_matrix), intent(in) :: a
    real(dp), intent(in) :: x(:)
    real(dp), intent(out) :: y(:)
    integer :: i, k

    do concurrent (i = 1:a%n)
      y(i) = 0.0_dp
      do k = a%row_ptr(i), a%row_ptr(i+1) - 1
        y(i) = y(i) + a%val(k) * x(a%col_idx(k))
      end do
    end do
  end subroutine matvec

  subroutine destroy(a)
    type(csr_matrix), intent(inout) :: a
    if (allocated(a%val)) deallocate(a%val, a%row_ptr, a%col_idx)
  end subroutine destroy

  recursive subroutine cg_solve(a, b, x, iters, info)
    type(csr_matrix), intent(in) :: a
    real(dp), intent(in) :: b(:)
    real(dp), intent(inout) :: x(:)
    integer, intent(out) :: iters, info
    real(dp), allocatable :: r(:), p(:), ap(:)
    real(dp) :: alpha, beta, rs_old, rs_new

    allocate(r(a%n), p(a%n), ap(a%n))
    call matvec(a, x, ap)
    r = b - ap
    p = r
    rs_old = dot_product(r, r)
    info = 0

    iterate: do iters = 1, MAX_ITER
      call a%apply(p, ap)
      alpha = rs_old / dot_product(p, ap)
      x = x + alpha * p
      r = r - alpha * ap
      rs_new = dot_product(r, r)
      if (sqrt(rs_new) < TOL .and. iters > 1) then
        exit iterate
      else if (rs_new /= rs_new .or. rs_new .GT. 1.0e300_dp) then
        info = -1
        return
      end if
      beta = rs_new / rs_old
      p = r + beta * p
      rs_old = rs_new
      if (verbose .AND. mod(iters, 100) == 0) &
        write(output_unit, '(A,I6,A,ES12.4)') 'iter ', iters, ' residual ', sqrt(rs_new)
    end do iterate

    if (iters > MAX_ITER .eqv. .true.) info = 1
    if (.not. (info .ne. 0) .neqv. .false.) print *, "converged"
    deallocate(r, p, ap)
  end subroutine cg_solve

end module sparse_solver

program main
  use sparse_solver
  implicit none
  type(csr_matrix) :: a
  real(kind=8), allocatable :: b(:), x(:)
  integer :: iters, info, ios
  complex :: z = (1.0, -2.5)
  character(len=64) :: line = "multi \
continued string"
  character(len=16) :: path = 'data/in.txt

  a%n = 3
  a%row_ptr = [1, 3, 6, 8]
  a%col_idx = [1, 2, 1, 2, 3, 2, 3]
  a%val = [4.0d0, -1.0d0, -1.0d0, 4.0d0, -1.0d0, -1.0d0, 4.0d0]
  allocate(b(3), x(3))
  b = [1.0, 2.0, 3.0]
  x = 0

  open(unit=10, file="out.dat", status='replace', action="write", iostat=ios)
  if (ios /= 0) stop "cannot open"
  call cg_solve(a, b, x, iters, info)
  select case (info)
  case (0)
    write(10, *) 'solution:', x
  case (1:)
    write(10, *) "no convergence after", iters
  case default
    error stop 2
  end select
  close(10)

  where (x < 0.0) x = 0.0
  forall (i = 1:3) b(i) = abs(x(i)) ** 2 + real(z) * aimag(z)
  print '(3F10.4)', b
  print *, huge(1), tiny(1.0), epsilon(1.0d0), 1.5e3, 2.d-4, 42, 0x1F, 12abc
  print *, max(1, 2), min(3.0, 4.0), len_trim(line), trim(adjustl(path))
  print *, .TRUE., .false., x .Eq. b, 3 .lt. 4, 5 .LE. 6, 7 .ge. 8
  print *, iand(5, 3), ior(5, 3), ieor(5, 3), ishft(1, 4)
  goto 100
100 continue
	x = x / 2	! tab-separated comment
  $weird = %odd ; y = x // "concat" ; z => ptr
  INTEGER :: UPPER_CASE_DECL
  CALL MATVEC(A, X, B)
  END PROGRAM main
