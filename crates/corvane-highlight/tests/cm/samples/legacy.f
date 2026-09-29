C     LEGACY.F -- FIXED-FORM FORTRAN 77 GAUSSIAN ELIMINATION
C     COLUMN-1 'C' COMMENTS ARE NOT COMMENTS TO THE MODE
*     NEITHER ARE ASTERISK COMMENTS
      PROGRAM GAUSS
      IMPLICIT DOUBLE PRECISION (A-H, O-Z)
      PARAMETER (N = 3, EPS = 1.0D-12)
      DIMENSION A(N,N), B(N), X(N)
      COMMON /WORK/ A, B, X
      DATA A /2.D0, 1.D0, -1.D0,
     &        -3.D0, -1.D0, 2.D0,
     &        -2.D0, 1.D0, 2.D0/
      DATA B /8.D0, -11.D0, -3.D0/
      EXTERNAL SWAPR
      INTRINSIC ABS, SQRT
      CHARACTER*20 TITLE
      LOGICAL DONE
      TITLE = 'GAUSS ELIM'
      DONE = .FALSE.
C
C     FORWARD ELIMINATION
C
      DO 30 K = 1, N - 1
         IPIV = K
         DO 10 I = K + 1, N
            IF (ABS(A(I,K)) .GT. ABS(A(IPIV,K))) IPIV = I
   10    CONTINUE
         IF (IPIV .NE. K) CALL SWAPR(A, B, N, K, IPIV)
         IF (ABS(A(K,K)) .LT. EPS) THEN
            WRITE (6, 900) K
            STOP 1
         ENDIF
         DO 20 I = K + 1, N
            F = A(I,K) / A(K,K)
            DO 15 J = K, N
               A(I,J) = A(I,J) - F * A(K,J)
   15       CONTINUE
            B(I) = B(I) - F * B(K)
   20    CONTINUE
   30 CONTINUE
C
C     BACK SUBSTITUTION
C
      DO 50 I = N, 1, -1
         S = B(I)
         DO 40 J = I + 1, N
            S = S - A(I,J) * X(J)
   40    CONTINUE
         X(I) = S / A(I,I)
   50 CONTINUE
      DONE = .TRUE.
      IF (DONE .AND. .NOT. .FALSE.) GO TO 60
      PAUSE 'UNREACHABLE'
   60 WRITE (6, 910) TITLE, (X(I), I = 1, N)
      ASSIGN 70 TO LABEL
   70 FORMAT (1X, 'DONE')
  900 FORMAT (1X, 'SINGULAR AT ROW ', I3)
  910 FORMAT (1X, A20, 3F12.6)
      END

      SUBROUTINE SWAPR(A, B, N, K, L)
      DOUBLE PRECISION A(N,N), B(N), T
      INTEGER N, K, L, J
      DO 10 J = 1, N
         T = A(K,J)
         A(K,J) = A(L,J)
         A(L,J) = T
   10 CONTINUE
      T = B(K)
      B(K) = B(L)
      B(L) = T
      RETURN
      END

      REAL FUNCTION AREA(R)
      REAL R
      AREA = 3.14159 * R ** 2
      ENTRY PERIM(R)
      RETURN
      END
      BLOCK DATA INIT
      COMMON /CONST/ PI
      DATA PI /3.1415926535/
      END
      MSG = "UNTERMINATED STRING
      MSG2 = 'ESCAPED \' QUOTE'
      MSG3 = 'TRAILING BACKSLASH \
      STILL IN STRING'
      I = 0 ! INLINE COMMENT
