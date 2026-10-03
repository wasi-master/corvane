/*
 * hello.c — a small ring buffer and some string helpers.
 * Ünïcödé in comments: naïve café, 日本語テキスト.
 */
#include <stdio.h>
#include <stdlib.h>
#include "ring.h"   // local header
#define MAX(a, b) ((a) > (b) ? (a) : (b))
#define RING_INIT(r, n) do { \
	(r)->head = 0;            \
	(r)->tail = 0; /* reset */ \
	(r)->cap = (n);           \
} while (0)
#   ifdef DEBUG
#     define LOG(fmt, ...) fprintf(stderr, fmt "\n", __VA_ARGS__)
#   else
#     define LOG(fmt, ...) ((void)0)
#   endif
#pragma once
#define CONTINUED \

int after_continuation;

typedef unsigned long size_type;
typedef struct ring ring_t;

struct ring {
	char *buf;
	size_t head, tail;
	size_t cap;
	volatile int flags;
	uint8_t bytes[16];
};

enum color { RED = 0x1F, GREEN = 0b1010, BLUE = 017 };
union value { int i; float f; double d; };

static const char *names[] = { "zero", "one", "two\tthree", "esc \"quoted\"", NULL };
static char sep = ',', nl = '\n', quote = '\'', hex = '\x41';
const char *long_line = "first half \
second half";

extern int errno;
register int counter;
_Bool is_ready = false;
__attribute__((unused)) static int __internal_flag = 1;
int café = 3, naïve_count;

int ring_push(ring_t *r, char c)
{
	if (r->tail - r->head >= r->cap)
		return -1;
	r->buf[r->tail++ % r->cap] = c;
	return 0;
}

static inline size_t
ring_len(const struct ring *r)
{
	return r->tail - r->head;
}

struct ring *
ring_new(size_t cap);

unsigned char *make_buffer(size_t n), other;
int values[4] = {1, 2, 3, 4}, (*fp)(int);
long table[8]
compute(void);

double average(const int *values, size_t n) {
	long long sum = 0LL;
	unsigned u = 42u;
	float ratio = 1.5f, small = .25, big = 6.02e23, neg = 1e-9;
	for (size_t i = 0; i < n; ++i) {
		sum += values[i];
	}
	switch (n) {
	case 0:
		return 0.0;
	default:
		break;
	}
	goto done;
done:
	return n ? (double)sum / n : 0.0;
}

int main(int argc, char **argv)
{
	ring_t r;
	RING_INIT(&r, 64);
	char *msg = "unterminated string
	int x = 10 / 2 % 3, y = x << 2 | 1 & ~0;
	x += y; x -= 1; x *= 2; x /= 3; x ^= 4;
	if (x != y && !(x == y) || x >= 0)
		printf("%d %s\n", x, argv[0]);
	while (argc-- > 0) { puts(*argv++); }
	int *p = &x, **pp = &p;
	size_t sz = sizeof(int) * 4;
	/* multi-line
	   comment spanning
	   lines */ int after = 1;
	x = x */* sneaky */2;
	#define INNER 1
	asm volatile ("nop");
	return EXIT_SUCCESS;
}
/* unterminated comment at end of file
