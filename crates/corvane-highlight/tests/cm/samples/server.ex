defmodule MyApp.Server do
  @moduledoc """
  A GenServer that keeps a counter and a cache.
  Supports "quoted" words and #{interpolation} inside docs.
  """
  use GenServer
  require Logger
  alias MyApp.{Cache, Metrics}
  import Enum, only: [map: 2, reduce: 3]

  @timeout 5_000
  @max_size 0x1F_FF
  @flags 0b1010
  @perm 0o755
  @octal 0755
  @ratio 3.14e-2
  @big 1_000_000.5E+10
  @type state :: %{count: integer(), cache: map()}

  defstruct count: 0, cache: %{}, name: nil

  ## Client API

  def start_link(opts \\ []) do
    name = Keyword.get(opts, :name, __MODULE__)
    GenServer.start_link(__MODULE__, opts, name: name)
  end

  def increment(pid, by \\ 1) when is_integer(by) and by > 0 do
    GenServer.call(pid, {:increment, by}, @timeout)
  end

  def get(pid), do: GenServer.call(pid, :get)

  defp log(msg), do: Logger.info("server: #{msg}")

  ## Callbacks

  @impl true
  def init(opts) do
    state = %__MODULE__{count: Keyword.get(opts, :initial, 0)}
    {:ok, state, {:continue, :warm_up}}
  end

  @impl GenServer
  def handle_call({:increment, by}, _from, %{count: count} = state) do
    new_state = %{state | count: count + by}
    {:reply, new_state.count, new_state}
  end

  def handle_call(:get, _from, state), do: {:reply, state.count, state}

  def handle_info(:tick, state) do
    state
    |> Map.update!(:count, &(&1 + 1))
    |> tap(fn s -> log("tick #{s.count} at #{inspect(DateTime.utc_now())}") end)
    |> then(&{:noreply, &1})
  end

  def handle_info(msg, state) do
    Logger.warning("unexpected message: #{inspect msg}")
    {:noreply, state}
  end

  def classify(value) do
    cond do
      is_nil(value) -> :empty
      value == true or value == false -> :boolean
      is_binary(value) and byte_size(value) > 0 -> :string
      true -> :other
    end
  end

  def parse(input) do
    case Integer.parse(input) do
      {n, ""} -> {:ok, n}
      {_n, rest} -> {:error, "trailing: " <> rest}
      :error -> {:error, :invalid}
    end
  end

  def safe_div(a, b) do
    try do
      a / b
    rescue
      ArithmeticError -> :infinity
    catch
      :exit, reason -> {:exit, reason}
    after
      Logger.debug('done dividing')
    end
  end

  def pipeline(list) do
    for x <- list, rem(x, 2) == 0, into: [] do
      x * x
    end
    |> Enum.filter(fn x -> x > 10 end)
    |> Enum.sum()
  end

  def with_example(map) do
    with {:ok, a} <- Map.fetch(map, :a),
         {:ok, b} <- Map.fetch(map, :b) do
      {:ok, a + b}
    else
      :error -> {:error, :missing}
    end
  end

  def sigils do
    words = ~w(alpha beta gamma)a
    regex = ~r/^[a-z]+\d*$/i
    chars = ?a + ?\n + ?\s + ?\C-a + ?é
    atoms = [:"quoted atom", :'single', :ok?, :done!, :<>, :<<, :+, :&&, :"with #{interp}"]
    str = "tab\t newline\n quote\" escape\\ unicode: héllo ✓"
    single = 'charlist with \'escape\''
    pct = %w{one #{two} three}
    pct2 = %Q[interpolated #{1 + 2} here]
    pct3 = %s(an atom literal)
    pct4 = %r{regex/(\d+)}
    pct5 = %q<no interp #{here}>
    ratio = total / count
    re = /(foo|bar)/
    unbalanced = a / (b / c
    {words, regex, chars, atoms, str, single, pct, pct2, pct3, pct4, pct5, ratio, re}
  end

  def vars do
    $stdout
    $1
    @@class_var
    @attr_é
    nested = "outer #{"inner #{deep} value"} end"
    at = "email #@user and #$global here"
    multi = "starts here
      and continues
      until here"
    heredoc = <<EOF
    raw text line
EOF
    x = 1..10
    y = x |> Enum.to_list |> length
    z = a <> b <<< 2 ~> c =~ d != e === f
    fun = fn a, b -> a + b end
    fun.(1, 2)
    obj.field.nested
    :erlang.system_time()
    receive do
      {:msg, payload} -> payload
    after
      1_000 -> :timeout
    end
  end
end

"""
A top-level triple-quoted block at column zero reads as a comment.
defmodule Nope do end
"""

defprotocol MyApp.Printable do
  def print(data)
end

defimpl MyApp.Printable, for: Integer do
  def print(n), do: IO.puts("int: #{n}")
end

defmacro unless_(clause, do: expression) do
  quote do
    if !unquote(clause), do: unquote(expression)
  end
end

if Mix.env() == :test, do: IO.puts("testing")
unless System.get_env("CI") do
	IO.puts("tab-indented line")
end
"""inline triple quote followed by text
list.each do |item, idx| item + idx end
list.map { |x| x * 2 }
pct6 = %w<a b c< and %x!cmd! %W|x #{y}| %= and %
calls = obj.method?.other! |> Kernel.+(1)
nested = %w{outer {inner} #{a{b}c} tail} rest
esc = "line \
continued"
# Trailing comment with ünïcødé ✓
"unterminated string at the end
