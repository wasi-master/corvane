--!strict
-- module.luau: typed Roblox-style module
--[[
	Luau supports type annotations, generics and string interpolation.
]]

local ReplicatedStorage = game:GetService("ReplicatedStorage")
local Players = game:GetService("Players")

export type Vector = { x: number, y: number, z: number? }
type Callback<T> = (value: T, index: number) -> boolean
type Shape = "circle" | "square" | "triangle"
type Result<T, E> = { ok: true, value: T } | { ok: false, error: E }
type Point = Vector & { label: string }
type Maybe = string?

local Module = {}
Module.__index = Module

export type Module = typeof(setmetatable({} :: {
	name: string,
	items: { [string]: number },
	onChanged: Callback<number>?,
}, Module))

function Module.new(name: string, capacity: number?): Module
	local self = setmetatable({}, Module)
	self.name = name
	self.items = {}
	self.capacity = capacity or 16
	return self
end

function Module:add(key: string, amount: number): ()
	local current = self.items[key] or 0
	self.items[key] = current + amount
	if self.onChanged then
		self.onChanged(current + amount, 1)
	end
end

function Module:describe(): string
	local count = 0
	for _, v in self.items do
		count += v
	end
	return `Module {self.name} has {count} items`
end

local greeting = `Hello, {Players.LocalPlayer.Name}!
	This interpolated string spans
	several lines until the backtick`

local function map<T, U>(list: { T }, f: (T) -> U): { U }
	local out: { U } = table.create(#list)
	for i, item in ipairs(list) do
		out[i] = f(item)
	end
	return out
end

local function clamp(value: number, min: number, max: number): number
	return math.clamp(value, min, max)
end

local position = Vector3.new(1, 2.5, -3)
local size = UDim2.fromScale(0.5, 0.25)
local offset = Vector2.new(10, 20) :: Vector2
local frame = UDim.new(0, 8)

local ok = pcall(function(): ()
	local value = if position.X > 0 then "positive" else "negative"
	print(value, typeof(value))
end)

local shape: Shape = "circle"
local flag = true and not false
local n = 0b1010 + 0x_FF + 1_000_000
local s = "unicode ✓ ünïcødé 日本語"
local x, y = clamp(5, 0, 10), map({ 1, 2, 3 }, function(v) return v * 2 end)
local continueFlag = shared.flag == nil or _G.debug ~= nil
local café: number = 1
local 名前 = "name"

task.spawn(function()
	while task.wait(1) do
		if not Module then break end
	end
end)

local unfinished = `this backtick never closes
