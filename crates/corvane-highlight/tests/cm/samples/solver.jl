# solver.jl — a small ODE / linear-algebra toolkit
#=
Block comment spanning lines.
  #= nested block comment =#
still inside the outer comment
=#
#= single-line block =# x = 1

module Solver

using LinearAlgebra, Statistics
import Base: show, +, *, getindex
export solve, Problem, rk4

const TOLERANCE = 1e-8
const MAX_ITERS = 10_000
const HEX = 0xFF_FF + 0x1p3 + 0x.8p-1
const BIN = 0b1010_0101
const OCT = 0o755
const FLOATS = (1.5, .25, 3., 2.5e10, 1.0f0, 6.02E+23, 1_000.000_1)
const COMPLEX = 3 + 4im
const RANGE = 1:10
const DOTS = a...
const TOO_MANY = x....
const INF_NAN = (Inf, NaN, nothing, true, false)

abstract type AbstractProblem end

struct Problem{T<:Real, N} <: AbstractProblem
    f::Function
    u0::Vector{T}
    tspan::Tuple{T,T}
    params::Dict{Symbol, Any}
end

mutable struct State{T}
    t::T
    u::Vector{T}
    history::Vector{Vector{T}}
end

primitive type Byte8 8 end

function Problem(f, u0::AbstractVector, tspan; kwargs...)
    Problem{eltype(u0), length(u0)}(f, collect(u0), tspan, Dict(kwargs))
end

"""
    rk4(prob; dt=0.01)

Integrates `prob` with the classic Runge–Kutta method.
"""
function rk4(prob::Problem{T}; dt::T=T(0.01)) where {T}
    t0, t1 = prob.tspan
    u = copy(prob.u0)
    ts = collect(t0:dt:t1)
    out = similar(ts, Vector{T})
    for (i, t) in enumerate(ts)
        k1 = prob.f(u, t)
        k2 = prob.f(u .+ dt / 2 .* k1, t + dt / 2)
        k3 = prob.f(u .+ dt / 2 .* k2, t + dt / 2)
        k4 = prob.f(u .+ dt .* k3, t + dt)
        u = u .+ dt / 6 .* (k1 .+ 2k2 .+ 2k3 .+ k4)
        out[i] = copy(u)
    end
    return ts, out
end

macro timed_block(label, ex)
    quote
        local t0 = time_ns()
        local val = $(esc(ex))
        println($(esc(label)), ": ", (time_ns() - t0) / 1e6, " ms")
        val
    end
end

Base.show(io::IO, p::Problem) = print(io, "Problem(", length(p.u0), " dims)")

function solve(A::Matrix{Float64}, b::Vector{Float64})::Vector{Float64}
    @assert size(A, 1) == size(A, 2) "A must be square"
    x = A \ b
    r = norm(A * x - b)
    r < TOLERANCE || @warn "residual too large" r
    return x
end

transpose_demo(M) = M' * M + M'' - M
arrays = [1 2 3; 4 5 6]
first_last = arrays[end, begin:end]
slice = arrays[end-1:end]
gen = (x^2 for x in 1:10 if isodd(x))
comp = [i + j for i in 1:3, j in 1:3]
nested = [[1, 2], [3, [4, 5]]]
tuple = (a = 1, b = "two", c = :three)

symbols = (:sym, :another_one!, :+, :(==), :<=, :..., Symbol("x"))
chars = ('a', '\n', '\t', '\\', '\'', '\x41', '\101', '\u00e9', '\U1F600', 'é', '😀', 'ab')
strings = ("plain", "with \"escapes\" and \$dollar", "interp $x and $(x + 1)",
           raw"C:\path\n", r"^\d+$"i, b"bytes", v"1.2.3", `echo cmd`)
triple = """
    multi-line string
    with "quotes" inside
    """
unicode = "héllo wörld — ünïcødé ✓ 日本語"
αβγ = 1.5; δ² = αβγ^2; x₁ = √2 ∘ sin
∑ = sum(x -> x ÷ 2, 1:10)
op_mix = a ∈ b && c ∉ d || e ≤ f ≥ g ≠ h ⊆ i × j ⋅ k ∩ l ∪ m
cmp = a == b != c === d !== e <: f >: g => h
bits = a << 2 >> 1 >>> 3 & b | c ⊻ d
update = (x += 1; y -= 2; z *= 3; w /= 4; v //= 5; u ^= 2; t |= 1; s &= 0)
pipe = data |> filter(isodd) |> sum
ternary = cond ? "yes" : "no"
anon = (x, y) -> x + y
isa_check = x isa Int && y in (1, 2) && in(3, s) && isa(z, Real)
splat = f(args...; kw...)
interp = :(a + $b)
ann = x::Int
param = Vector{Union{Missing, Float64}}(undef, 3)
nested_param = Dict{String, Vector{Tuple{Int, Int}}}()
typed_field = (p.f)::Function
dollar = $x
at_ops = @. a + b
q = quote x end

if length(ARGS) > 0
    println("args: ", join(ARGS, ", "))
elseif isempty(ENV)
    nothing
else
	@info "tab-indented" key=value
end

let counter = 0
    while counter < 3
        global counter += 1
        counter == 2 && continue
    end
end

try
    error("boom")
catch err
    @error "caught" exception = (err, catch_backtrace())
finally
    flush(stdout)
end

x = ; y = @ ; z = ¬foo
end # module
'unterminated
old_transpose = M.' * M
empty_char = '' + 1
"unterminated string
continues here
