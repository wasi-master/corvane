#### Docco title comment: app.coffee
# Regular single-line comment with unicode — café ✓

###
Block comment spanning lines
  with # inner hashes and ## more
###

### single-line block comment ###
x = 1 ### trailing block ### + 2
### unterminated block comment
continues here # still comment ## two
and ends ### after = "code"

{EventEmitter} = require 'events'
fs = require "fs"
path = require('path')

VERSION = '1.0.0'
DEBUG = on
QUIET = off
enabled = yes
disabled = no
nothing = null
missing = undefined
big = Infinity
bad = NaN
truth = true and not false

class Animal extends EventEmitter
  @count: 0
  constructor: (@name, @legs = 4) ->
    super()
    Animal.count++
    @emit 'created', this

  speak: (sound = "...") =>
    console.log "#{@name} says #{sound}"
    return @

  move: (meters) ->
    alert @name + " moved #{meters}m."
    @legs * meters

class Snake extends Animal
  move: ->
    alert "Slithering..."
    super 5

numbers = [1, 2.5, .75, 3., -4, -0.5, 0x1F, 0XAB, 1e10, 2.5e-3, 1.5E+2, 0, 007, 1..10]
range = [1...100]
slice = list[2..]
hex = 0xDEADbeef
octalish = 0o755
binaryish = 0b1010

square = (x) -> x * x
cube   = (x) -> square(x) * x
fat = (a, b) => a ** b // 2 % 3
assign = 10
assign += 1; assign -= 2; assign *= 3; assign /= 4; assign %= 5
assign **= 2
bits = a & b | c ^ ~d << 2 >> 1 >>> 3
bits &= 1; bits |= 2; bits ^= 3; bits <<= 1; bits >>= 1
logic = a && b || !c
logic ||= default_value
logic &&= other
logic ?= fallback
maybe = obj?.prop ? 'none'
cmp = a == b and c != d or e < f and g > h and i <= j or k >= l
check = a is b and c isnt d
member = 'x' in list and item of object
kind = typeof value
inst = value instanceof Array

if happy and knowsIt
  clapsHands()
  chaChaCha()
else if sad
  cry()
else
  showIt()

unless done then work() else rest()

date = if friday then sue else jill

while count < 10
  count++
until finished
  step()
loop
  break if tired
  continue

for food in ['toast', 'cheese', 'wine']
  eat food
for own key, value of object
  console.log "#{key}: #{value}"
for i in [0..10] by 2 when i isnt 4
  print i

switch day
  when "Mon" then go work
  when "Tue" then go relax
  when "Thu", "Fri"
    go iceFishing
  else go home

try
  allHellBreaksLoose()
  catsAndDogsLivingTogether()
catch error
  print error
finally
  cleanUp()

throw new Error "Oops"
debugger
delete obj.key
do (x = 1) -> x

single = 'single \'escaped\' quotes with "double" inside'
double = "double \"escaped\" with 'single' and #{interp + 1}"
heredoc = '''
  Heredoc with 'quotes' and "doubles"
  spanning lines
  '''
herestring = """
  Interpolated #{heredoc} here
  <strong>html</strong>
  """
unterminated = "runs across
lines until here"
escaped_end = 'backslash at end \
continues'

regex = /^[a-z]+\d*$/gi
division = total / count / 2
notregex = a / b
heregex = ///
  ^(\d+)   # digits
  \.       # dot
///g
slash_in_regex = /\/path\/to/
unterminated_regex = /abc
after = 1

obj =
  name: 'Corvane'
  nested:
    deep: [1, 2, 3]
    fn: -> @name
  'quoted-key': true
  "double-key": false

matrix = [
  [1, 0, 0]
  [0, 1, 0]
  [0, 0, 1]
]

call(arg1, arg2)
  .then (result) -> result.value
  .catch (err) -> console.error err
chain = a.b.c.d()
prop = obj.class.if.for
at = @prop and @$special and @_under
at_digit = @0
bare = @
embedded = `function() { return 42; }`
splat = (args...) -> args.length
[first, rest...] = list
{a, b: {c}} = obj
existential = foo?
soak = foo?.bar?()?.baz
comprehension = (x * 2 for x in [1..5] when x % 2 is 0)
names = (name for own name of dict)

	tab-indented line
weird = 5 ¤ 3
emoji = "🎉 party"
$dollar = $.ajax
_under = _.map
last = done ->
  return result

if ready then go()
  indented_after_then = 1
    deeper = 2
back = 0
while x then y
	tab_indent_at_root = true
fn = (a) -> return a
  after_return = 1
