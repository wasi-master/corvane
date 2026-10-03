# frozen_string_literal: true
# A small Sinatra-ish app — sample for the highlighter (naïve café ✓).

require 'json'
require_relative "lib/helpers"

=begin
Block comment: everything here is a comment,
even def and "strings".
=end
=begin not a block comment start
x = 1

module Shop
  VERSION = "1.2.3".freeze
  DEFAULTS = { currency: :usd, "locale" => 'en', rate: 1.5e-3 }

  class Item < Struct.new(:name, :price)
    include Comparable
    attr_accessor :tags, :qty
    @@count = 0
    $stdout.sync = true

    def initialize(name, price = 0, *rest, **opts, &block)
      super(name, price)
      @tags = opts.fetch(:tags) { [] }
      @qty ||= 0
      @@count += 1
      yield self if block_given?
    end

    def <=>(other) = price <=> other.price
    def ==(other); name == other.name; end
    def [](key) @tags[key] end
    def +(o) self.class.new(name, price + o.price) end
    def -@; self; end
    def to_s
      "#{name} (#{format('%.2f', price)}) x#@qty for #$0 #@@count"
    end
    def valid?; !name.nil? && price >= 0; end
    def save!; raise NotImplementedError, "abstract"; end
    def self.build(attrs) = new(**attrs)
  end

  def self.total(items)
    items.sum { |i| i.price * i.qty }
  end
end

numbers = [0, 42, 1_000, 3.14, 1e10, 2.5E-3, 0x1F, 0b1010, 0o17, 0777, 012]
words = %w[apple banana cherry]
syms = %i(a b c)
quoted = %q{single 'quoted' #{no interpolation}}
Quoted = %Q[double "quoted" #{1 + 2} done]
plain = %(parens #{nested(1)} and (balanced))
regex = %r{^/api/(\d+)/?$}i
cmd = %x(ls -la)
sym = %s(symbol)
angle = %<angle #{x}>
pipes = %|pipe delimited|
mod = 10 % 3
modeq = x %= 2
%w{#{not} interpolated}

pattern = /\A[a-z]+#{suffix}\z/mi
ratio = total / count / 2
path = "a/b".split(/\//)
if line =~ /^(\w+):\s*(.*)$/ then puts $1, $2, $~ end
half = width / 2; quarter = width/4
str = 'it\'s' + "say \"hi\"\n" + `echo #{cmd}` + "tab\t"
char = ?a + ?\n + ?\C-a + ?\M-\C-x
x = cond ? a : b

html = <<~HTML
  <div class="#{klass}">
    #{content}
  </div>
HTML
sql = <<-SQL
    SELECT * FROM items WHERE id = #{id}
    SQL
raw = <<~'RAW'
  no #{interpolation} here
RAW
shell = <<~`CMD`;
  ls
CMD
not_heredoc = foo(<<~EOS)
second = x << 2 << 3

:symbol; :"quoted #{sym}"; :'single'; :+; :-; :<=>; :<<; :>; :[]; :!; :a?; :b!; :c=
Foo::Bar::BAZ; ::TopLevel; obj&.name; a->(x) { x }
hash = { key: 1, 'str': 2, "k" => 3, sym => 4 }
fn = ->(a, b = 2) { a + b }
fn2 = lambda { |x, (y, z), *w| x + y }
[1, 2, 3].each_with_index.map do |value, index|
  value * index
end.select { |v| v.even? }.reduce(0) { |s, v| s + v }

begin
  risky!
rescue ArgumentError, TypeError => e
  warn "failed: #{e.message}"
  retry if (tries += 1) < 3
ensure
  cleanup
end

case input
when Integer, Float then :number
when /\d+/ then :digits
when 1..10, 'a'...'z' then :range
in {name: String => name, age: Integer => age} if age > 18
  :adult
else nil
end

unless defined?(Rails) then puts __FILE__, __LINE__, __dir__ end
while i < 10 do i += 1 end
until done do step end
loop do break if stop? end
for x in 1..3 do next if x == 2; redo if false end
alias new_name old_name
undef old_name
private def helper; end
protected
public
puts "unterminated string
continues here"
x = "a #{ "b #{ c } d" } e"
y = "hash in #{ {a: 1}[:a] } string"
z = "brace in string } and { here"
BEGIN { puts :start }
END { puts :end }
$global = $1 + $: .size
@ivar; @@cvar; @_under; @ not_ivar
a = b ? c : d
obj.method arg, :sym
__END__
data after end

café = "naïve #{wört} ünïcode"; :café; @größe = 1
	indented_with_tab = { "κλειδί" => 'τιμή' }
def résumé(x) = x
