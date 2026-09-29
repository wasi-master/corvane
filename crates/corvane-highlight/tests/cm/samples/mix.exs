defmodule MyApp.MixProject do
  use Mix.Project

  @version "0.4.1"
  @source_url "https://github.com/example/my_app"

  def project do
    [
      app: :my_app,
      version: @version,
      elixir: "~> 1.15",
      start_permanent: Mix.env() == :prod,
      deps: deps(),
      aliases: aliases(),
      elixirc_paths: elixirc_paths(Mix.env()),
      test_coverage: [tool: ExCoveralls, threshold: 85.5],
      preferred_cli_env: [coveralls: :test, "coveralls.html": :test],
      docs: [
        main: "readme",
        source_ref: "v#{@version}",
        extras: ~w(README.md CHANGELOG.md)
      ],
      dialyzer: [plt_add_apps: [:mix, :ex_unit], flags: [:unmatched_returns]]
    ]
  end

  # Configuration for the OTP application.
  def application do
    [
      mod: {MyApp.Application, []},
      extra_applications: [:logger, :runtime_tools, :crypto]
    ]
  end

  defp elixirc_paths(:test), do: ["lib", "test/support"]
  defp elixirc_paths(_), do: ["lib"]

  defp deps do
    [
      {:phoenix, "~> 1.7.10"},
      {:ecto_sql, "~> 3.11"},
      {:postgrex, ">= 0.0.0"},
      {:jason, "~> 1.4"},
      {:telemetry_metrics, "~> 0.6", override: true},
      {:credo, "~> 1.7", only: [:dev, :test], runtime: false},
      {:dialyxir, "~> 1.4", only: :dev, runtime: false},
      {:ex_doc, ">= 0.0.0", only: :dev, runtime: false},
      {:excoveralls, "~> 0.18", only: :test},
      {:local_dep, path: "../local_dep"},
      {:git_dep, git: "https://github.com/example/dep.git", tag: "v1.0"}
    ]
  end

  defp aliases do
    [
      setup: ["deps.get", "ecto.setup"],
      "ecto.setup": ["ecto.create", "ecto.migrate", "run priv/repo/seeds.exs"],
      "ecto.reset": ["ecto.drop", "ecto.setup"],
      test: ["ecto.create --quiet", "ecto.migrate --quiet", "test"],
      lint: &lint/1
    ]
  end

  defp lint(args) do
    Mix.shell().info("Linting with #{length(args)} args…")
    {_, 0} = System.cmd("mix", ["credo", "--strict" | args], into: IO.stream())
    :ok
  end
end

defmodule Mix.Tasks.MyApp.Report do
  @shortdoc "Prints a dependency report"
  @moduledoc """
  Usage: `mix my_app.report [--only NAME] [--json]`
  """
  use Mix.Task

  @switches [only: :string, json: :boolean]

  @impl Mix.Task
  def run(argv) do
    {opts, _rest, invalid} = OptionParser.parse(argv, strict: @switches)
    unless invalid == [], do: Mix.raise("invalid options: #{inspect(invalid)}")

    Mix.Project.config()[:deps]
    |> Enum.map(fn
      {name, req} when is_binary(req) -> {name, req, []}
      {name, req, extra} when is_list(extra) -> {name, req, extra}
      {name, extra} -> {name, nil, extra}
    end)
    |> Enum.filter(&(opts[:only] in [nil, to_string(elem(&1, 0))]))
    |> render(Keyword.get(opts, :json, false))
  end

  defp render(deps, true), do: deps |> Map.new(fn {n, r, _} -> {n, r} end) |> Jason.encode!() |> IO.puts()

  defp render(deps, false) do
    for {name, req, extra} <- deps do
      only = Keyword.get(extra, :only, :all)
      IO.puts(String.pad_trailing("#{name}", 20) <> "#{req || "-"}\t#{inspect(only)}")
    end
    :ok
  end
end
