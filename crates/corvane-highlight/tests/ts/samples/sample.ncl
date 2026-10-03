# Service configuration with contracts, defaults and merging.
let Port = std.contract.from_predicate (fun p => std.is_number p && p > 0 && p < 65536) in

let Service = {
  name | String,
  port | Port | default = 8080,
  replicas | Number | default = 1,
  debug | Bool | default = false,
  tags | Array String | default = [],
  env | { _ : String } | default = {},
} in

let make_url = fun host port => "http://%{host}:%{std.string.from_number port}" in

let base = {
  name = "api",
  tags = ["backend", "public"],
  env = { LOG_LEVEL = "info", REGION = "eu-west-1" },
} | Service in

let services = [
  base,
  base & { port | force = 9090, debug | force = true },
] in

{
  services = services,
  urls = std.array.map (fun s => make_url "localhost" s.port) services,
  total_replicas = std.array.fold_left (fun acc s => acc + s.replicas) 0 services,
  mode = if std.array.length services > 1 then 'multi else 'single,
  banner = m%"
    Deployed %{std.string.from_number (std.array.length services)} services.
    Plain "quotes" need no escaping here.
  "%,
  greeting = "tab:\there, null: %{std.to_string null}",
  checked = std.array.all (fun s => s.port != 0) services,
}
