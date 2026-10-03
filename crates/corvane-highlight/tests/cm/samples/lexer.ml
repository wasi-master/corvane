(* lexer.ml — a tiny hand-written lexer for an expression language.
   Nested comments (* like this one *) keep going
   across several lines until the outer one closes. *)

(**)
(*)  "(*)" looks like a comment start in OCaml *)

open Printf
module SMap = Map.Make (String)

exception Lex_error of string * int

type token =
  | INT of int
  | FLOAT of float
  | IDENT of string
  | STRING of string
  | LPAREN | RPAREN
  | PLUS | MINUS | STAR | SLASH
  | EOF

type position = { mutable line : int; mutable col : int }

let keywords : (string, token) Hashtbl.t = Hashtbl.create 16

let rec skip_ws s i =
  if i < String.length s && (s.[i] = ' ' || s.[i] = '\t') then skip_ws s (i + 1)
  else i

let hex = 0xFF_ff and bin = 0b1011 and oct = 0o755 and big = 1_000_000
let floats = [ 3.14; 1e10; 2.5e-3; 6.02E+23; 0.; 7. ]
let weird = 0b12 + 0x1.5 + 0o78 + 123abc
let chars = [ 'a'; '\n'; '\''; '"' ]

let is_digit = function '0' .. '9' -> true | _ -> false
let is_alpha c = match c with 'a' .. 'z' | 'A' .. 'Z' | '_' -> true | _ -> false

let describe = function
  | INT n -> sprintf "INT(%d)" n
  | FLOAT f -> sprintf "FLOAT(%g)" f
  | IDENT s -> "IDENT(" ^ s ^ ")"
  | STRING s -> sprintf "STRING(%S)" s
  | LPAREN -> "(" | RPAREN -> ")"
  | PLUS -> "+" | MINUS -> "-" | STAR -> "*" | SLASH -> "/"
  | EOF -> "EOF"

let escaped = "tab\there, quote \" inside, backslash \\ done"
let multi = "this string
spans two lines"
let ends_in_backslash = "line one \
                         line two"
let quoted = {|raw string with "quotes" and \n no escapes|}
let quoted_multi = {|first line
second line |} and after = 1
let labelled ~name ?(greeting = "hello") () = greeting ^ ", " ^ name
let poly = `Red | `Green | `Blue_ish
let _ = labelled ~name:"Zoë" ?greeting:(Some "hi") ()

let tokenize (src : string) : token list =
  let len = String.length src in
  let rec go i acc =
    if i >= len then List.rev (EOF :: acc)
    else
      match src.[i] with
      | ' ' | '\t' | '\n' -> go (i + 1) acc
      | '(' -> go (i + 1) (LPAREN :: acc)
      | ')' -> go (i + 1) (RPAREN :: acc)
      | '+' -> go (i + 1) (PLUS :: acc)
      | '-' -> go (i + 1) (MINUS :: acc)
      | '*' -> go (i + 1) (STAR :: acc)
      | '/' -> go (i + 1) (SLASH :: acc)
      | c when is_digit c ->
          let j = ref i in
          while !j < len && is_digit src.[!j] do incr j done;
          go !j (INT (int_of_string (String.sub src i (!j - i))) :: acc)
      | c when is_alpha c ->
          let j = ref (i + 1) in
          while !j < len && (is_alpha src.[!j] || is_digit src.[!j]) do
            incr j
          done;
          go !j (IDENT (String.sub src i (!j - i)) :: acc)
      | c -> raise (Lex_error (sprintf "unexpected %C" c, i))
  in
  go 0 []

class counter init = object (self)
  val mutable count = init
  method incr = count <- count + 1; self#get
  method get = count
  initializer print_endline "counter ready"
end

module type STACK = sig
  type 'a t
  val empty : 'a t
  val push : 'a -> 'a t -> 'a t
end

module Stack : STACK = struct
  type 'a t = 'a list
  let empty = []
  let push x s = x :: s
end

let () =
  let bits = (5 land 3) lor (1 lsl 4) lxor (8 lsr 1) asr 1 mod 3 in
  assert (bits >= 0 || false);
  for i = 10 downto 1 do printf "%d " i done;
  for i = 1 to 3 do ignore i done;
  try failwith "boom" with Failure msg -> print_string msg;
  let lazy_v = lazy (42 * 2) in
  ignore (Lazy.force lazy_v);
  if not true then exit 1 else raise_notrace Exit

external caml_hash : int -> int = "caml_hash"
let (|>>) x f = f x
let x = 1 |>> succ @ [] <> [2] := !r
let ünïcödé_name = "日本語 ✓"
let emoji = "😀" ^ "🚀"
	let tabbed = 1	(* after a tab *)
let unit_v : unit = ()
let f : int -> float -> bool -> char -> string = fun _ _ _ _ _ -> ""
let lst = List.map (fun x -> x * 2) [1; 2; 3]
(* unterminated comment at the end of the file (* nested
   still open
