# server.cr — a tiny HTTP JSON API in Crystal
# Ünïcode in comments: 日本語 ✓
require "http/server"
require "json"
require "./models/*"

module Api
  VERSION = "1.2.0"
  MAX_BODY = 1_048_576
  RATIO = 0.75
  SCI = 6.02e23
  NEG = -1.5E-3
  HEX = 0xFF_FF
  OCT = 0o755
  BIN = 0b1010_0101
  ZERO = 0
  SUFFIXED = 42_i64 + 3.0_f32 + 7u8

  enum Status
    Active
    Suspended
    Deleted
  end

  struct Point
    getter x : Int32
    getter y : Int32

    def initialize(@x : Int32, @y : Int32)
    end

    def +(other : Point) : Point
      Point.new(x + other.x, y + other.y)
    end

    def ==(other)
      x == other.x && y == other.y
    end

    def [](index)
      index == 0 ? x : y
    end

    def empty?
      x.zero? && y.zero?
    end

    def reset!
      @x = 0
    end
  end

  abstract class Handler
    abstract def call(ctx : HTTP::Server::Context) : Nil
  end

  class UserHandler < Handler
    include JSON::Serializable
    @@count = 0
    @users = {} of String => User
    property name : String?
    private getter lock = Mutex.new

    def call(ctx)
      @@count += 1
      case ctx.request.method
      when "GET"
        ctx.response.content_type = "application/json"
        ctx.response.print @users.values.to_json
      when "POST"
        body = ctx.request.body.try(&.gets_to_end) || ""
        user = User.from_json(body)
        @users[user.id] = user
        ctx.response.status_code = 201
      else
        ctx.response.status = :method_not_allowed
      end
    rescue ex : JSON::ParseException
      ctx.response.respond_with_status(400, "bad json: #{ex.message}")
    ensure
      Log.info { "handled #{@@count} requests" }
    end
  end

  lib LibC
    fun getpid : Int32
    fun strlen(s : UInt8*) : SizeT
  end

  macro define_getter(name)
    def {{name.id}}
      @{{name.id}}
    end
  end

  macro def_twice(*names)
    {% for name in names %}
      def {{name.id}}_twice; {{name}} * 2; end
    {% end %}
  end

  macro def build
  end

  union IntOrFloat
    int : Int32
    float : Float64
  end

  alias Callback = Proc(Int32, Nil)
end

def greet(name : String, greeting = "Hello") : String
  "#{greeting}, #{name.capitalize}! You have #{1 + 2} messages."
end

symbols = [:ok, :not_found, :"quoted symbol", :+, :<=>, :[]?, :Name]
chars = ['a', '\n', '\'', '\u{1F600}', 'é']
escaped = "tab\t quote\" backslash\\ interp \#{not}"
raw = %q(no #{interpolation} here)
words = %w[alpha beta gamma]
interp = %(parens #{1 + 1} (nested))
angle = %<angle brackets>
braces = %{braces}
pipes = %|pipes|
regex = /^[a-z]+\d*$/i
regex2 = %r{^/api/v(\d+)/users$}
percent = 10 % 3
modulo_assign = 5
modulo_assign %= 2
range = (1..10).to_a + (1...5).to_a
lambda = ->(x : Int32) { x * 2 }
safe = nil.try &.size
flag = true || false && !nil
bits = (1 << 4) | (0xF0 >> 2) & ~3 ^ 5 ** 2
cmp = 1 <=> 2
match = "abc" =~ /b/
nomatch = "abc" !~ /z/
same = 1 === 1
tern = flag ? :yes : :no

text = <<-EOS
  Heredoc with #{interp} interpolation
  and a \n escape
  EOS
raw_doc = <<-'RAW'
  no #{interpolation} in here
  RAW

{% if flag?(:linux) %}
  puts "linux"
{% else %}
  puts "other"
{% end %}
puts {{ VERSION }}
puts "macro {{ "inside" }} string"
x = \{{ escaped_macro }}

@[Link("ssl")]
@[AlwaysInline]
def fast; end

server = HTTP::Server.new([Api::UserHandler.new])
address = server.bind_tcp "0.0.0.0", 8080
puts "Listening on http://#{address}"
spawn do
  loop do
    sleep 1.second
    break if Api::Status::Deleted.value > 2
  end
end
until done
  next unless ready?
end
while false; end
x = uninitialized Int32
sz = sizeof(Int32) + instance_sizeof(String)
ptr = pointerof(x)
yield_self = self
typeof(1)
__FILE__ + __DIR__
obj.class.name
obj.end
emoji = "😀 🚀"
astral = %😀abc😀 tail + 1
astral_char = '😀' + '😀x'
mixed = %€euro€ + %é(x)é
	tabbed = 1	# tab and comment
unterminated = "this string
continues onto the next line"
%r
still_regex
