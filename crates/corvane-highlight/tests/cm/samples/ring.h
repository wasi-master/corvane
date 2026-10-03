#ifndef RING_H
#define RING_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct ring ring_t;

int ring_push(ring_t *r, char c);
int ring_pop(ring_t *r, char *out);
size_t ring_len(const ring_t *r);
const char *ring_name(void);
static inline int ring_empty(const ring_t *r) { return ring_len(r) == 0; }

#define RING_OK 0 // success
#define RING_FULL -1 /* no room */

#ifdef __cplusplus
}
#endif

#endif /* RING_H */
