// A small particle simulation.
package sample

import "core:fmt"
import "core:math"

MAX_PARTICLES :: 256
GRAVITY : f32 : 9.81

Vec2 :: [2]f32
Kind :: enum u8 { Spark, Smoke, Dust }

Particle :: struct { pos, vel: Vec2, life: f32, kind: Kind }

/* Advance every live particle by dt seconds. */
update :: proc(particles: []Particle, dt: f32) -> (alive: int) {
    for &p in particles {
        if p.life <= 0 do continue
        p.vel.y -= GRAVITY * dt
        p.pos += p.vel * dt
        p.life -= dt
        alive += 1
    }
    return
}

describe :: proc(k: Kind) -> string {
    switch k {
    case .Spark: return "spark"
    case .Smoke, .Dust: return "cloud"
    }
    return "unknown"
}

main :: proc() {
    particles := make([dynamic]Particle, 0, MAX_PARTICLES)
    defer delete(particles)
    for i in 0..<8 {
        angle := f32(i) * math.PI / 4
        append(&particles, Particle{vel = Vec2{math.cos(angle), math.sin(angle)} * 5, life = 1.5, kind = .Spark})
    }
    alive := update(particles[:], 0.016)
    fmt.printf("%d alive, first is a %s\n", alive, describe(particles[0].kind))
    when ODIN_DEBUG { fmt.println("debug build", true) }
}
