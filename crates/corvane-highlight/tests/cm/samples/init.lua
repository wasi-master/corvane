#!/usr/bin/env lua
-- init.lua: bootstraps the plugin manager and editor settings
--[[
  Multi-line comment spanning lines.
  It ends on the next line ]] local after = "code after comment"
--[==[ long bracket with level: only ]] ends it in this mode ]==]

local M = {}
local vim = vim
local fn, api = vim.fn, vim.api
local uv = vim.loop or vim.uv

M.version = "1.4.2"
M.count = 0
M.ratio = 3.14159
M.hex = 0xFF + 0x1f
M.exp = 1e10 + 2.5E-3 + 6.02e23
M.frac = .5 + 5. + 12.34.56
M.mixed = 123abc
M.unicode = "héllo wörld — ünïcødé ✓"
M.name_é1 = 42 + é2

local function trim(s)
	return (s:gsub("^%s*(.-)%s*$", "%1"))
end

local function split(str, sep)
	local out = {}
	for piece in string.gmatch(str, "([^" .. sep .. "]+)") do
		table.insert(out, piece)
	end
	return out
end

function M.setup(opts)
  opts = opts or {}
  local defaults = { enabled = true, debug = false, level = 2 }
  for k, v in pairs(defaults) do
    if opts[k] == nil then
      opts[k] = v
    elseif type(opts[k]) ~= type(v) then
      error(("bad option %s: expected %s"):format(k, type(v)))
    end
  end
  M.opts = setmetatable(opts, { __index = defaults })
  return M
end

function M:increment(n)
  self.count = self.count + (n or 1)
  if self.count >= 10 and not self.silent then
    print('count reached ' .. tostring(self.count))
  end
  return self
end

local Stack = {}
Stack.__index = Stack

function Stack.new()
  return setmetatable({ items = {}, size = 0 }, Stack)
end

function Stack:push(value)
  self.size = self.size + 1
  self.items[self.size] = value
end

function Stack:pop()
  if self.size <= 0 then return nil end
  local v = self.items[self.size]
  self.items[self.size] = nil
  self.size = self.size - 1
  return v
end

local escapes = "tab\tnewline\nquote\"backslash\\ end"
local single = 'it\'s a \'quoted\' string'
local unterminated = "this string never closes
local unterminated2 = 'nor does this one
local trailing = "ends with a backslash\
local empty, empty2 = "", ''
local long = [[
  a long string
  with "quotes" inside
]]

local t = { 1, 2, 3; x = 1, ["key with spaces"] = true, [10] = false }
print(#t, t[1] % 2, 2 ^ 8, 7 // 2, -t[2], a .. b, x < y, x > y)

repeat
  M.count = M.count - 1
until M.count <= 0

while true do
  local ok, err = pcall(function() return require("missing.module") end)
  if not ok then io.stderr:write(err, "\n") break end
end

for i = 1, 10, 2 do
  goto continue
  ::continue::
end

local co = coroutine.create(function(a, b)
  local c = coroutine.yield(a + b)
  return c * 2
end)
print(coroutine.resume(co, 1, 2))

local selfish = self_made
local mt = getmetatable("") ; local _G = _G
os.exit(math.floor(math.random() * 100))
debug.traceback() package.path = package.path .. ";./?.lua"
local ending = trueish or falsehood or true or false

return M
--[[ an unterminated long comment at the end of the file
	still a comment
