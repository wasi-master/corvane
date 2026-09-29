// Package server implements a tiny HTTP job queue.
// Ünïcödé comments are fine: 日本語のコメント.
package server

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"net/http"
	"sync"
	"time"
)

/* A block comment
   spanning several lines, with * stars * and / slashes /
   and the end here */

const (
	maxJobs    = 1_000
	hexMask    = 0xFF_FF
	upperHex   = 0XdeadBEEF
	octalPerm  = 0644
	octalNew   = 0o755
	binaryBits = 0b1010_0101
	pi         = 3.14159
	avogadro   = 6.022e23
	tiny       = 1e-9
	half       = .5
	exp        = .25e+3
	trailing   = 1.
	imaginary  = 2.5i
	big        = 9_223_372_036_854_775_807
)

const (
	StateIdle State = iota
	StateRunning
	StateDone
)

type State int

// Job is one unit of work.
type Job struct {
	ID       int64             `json:"id"`
	Name     string            `json:"name,omitempty" db:"name"`
	Payload  []byte            `json:"-"`
	Tags     map[string]string `json:"tags"`
	Created  time.Time
	priority uint8
	Ratio    float64
	Parts    complex128
	flags    uintptr
	r        rune
	Any      any
}

type Number interface {
	~int | ~int64 | ~float64
}

// Sum adds up a slice of numbers.
func Sum[T Number](xs []T) T {
	var total T
	for _, x := range xs {
		total += x
	}
	return total
}

type Pair[K comparable, V any] struct {
	Key   K
	Value V
}

type Queue struct {
	mu   sync.Mutex
	jobs chan *Job
	done <-chan struct{}
	errs chan<- error
}

var ErrFull = errors.New("queue is full")

func NewQueue(ctx context.Context) *Queue {
	q := &Queue{jobs: make(chan *Job, maxJobs), done: ctx.Done()}
	go q.loop(ctx)
	return q
}

func (q *Queue) loop(ctx context.Context) {
	ticker := time.NewTicker(500 * time.Millisecond)
	defer ticker.Stop()
outer:
	for {
		select {
		case job, ok := <-q.jobs:
			if !ok {
				break outer
			}
			q.run(job)
		case <-ticker.C:
			continue
		case <-ctx.Done():
			break outer
		default:
			time.Sleep(time.Millisecond)
		}
	}
}

func (q *Queue) run(job *Job) (err error) {
	defer func() {
		if r := recover(); r != nil {
			err = fmt.Errorf("job %d panicked: %v", job.ID, r)
		}
	}()
	switch {
	case job.priority > 10 && job.Ratio >= 0.5:
		fallthrough
	case job.priority<<2 != 0 || job.flags&^0x3 == 0:
		job.r = 'x'
	default:
		goto fail
	}
	return nil
fail:
	return ErrFull
}

func runes() []rune {
	return []rune{'a', '\n', '\'', '\\', '\x7f', 'é', '\U0001F600', 'ß', '日'}
}

func strs() []string {
	s := "plain \"quoted\" and \t tabs \\ backslash"
	u := "ünïcödé ✓ 日本"
	e := ""
	raw := `raw string with "quotes" and \n no escapes`
	multi := `first line
second line with 'quotes'
	indented third line
last line`
	return []string{s, u, e, raw, multi}
}

func continued() string {
	return "a string ending in a backslash \
continues on the next line"
}

func unterminated() string {
	x := "never closed
	y := 'z
	return x + y
}

func (q *Queue) Handle(w http.ResponseWriter, r *http.Request) {
	var job Job
	if err := json.NewDecoder(r.Body).Decode(&job); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}
	q.mu.Lock()
	defer q.mu.Unlock()
	select {
	case q.jobs <- &job:
		w.WriteHeader(http.StatusAccepted)
	default:
		http.Error(w, ErrFull.Error(), http.StatusServiceUnavailable)
	}
	n := len(q.jobs) + cap(q.jobs)
	m := copy(job.Payload, []byte("x"))
	delete(job.Tags, "old")
	c := complex(1, 2)
	println(n, m, real(c), imag(c), new(int), append([]int{}, 1))
	close(make(chan bool))
	if true && !false || nil == nil {
		panic("unreachable")
	}
	x := 5
	x %= 3
	x ^= 1
	x >>= 1
	x &= ^x
	x |= 0
	x--
	_ = x / 2 * 3 - 1
	a, b := 1, 2
	a, b = b, a
	var ptr *int = &a
	*ptr++
	if a := 0; a < b {
		_ = [...]string{"x", "y"}[0:1]
	}
	lbl := struct{ $weird int }{}
	_ = lbl
	# @ ~ are stray chars
}

/* an unterminated block comment at the end
of the file keeps going
