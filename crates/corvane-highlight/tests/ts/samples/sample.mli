(** A persistent priority queue keyed by integers.

    All operations are purely functional. *)

(* Plain comment: the implementation lives in sample.ml. *)

type priority = int
(** Lower numbers are served first. *)

type 'a t
type error =
  | Empty
  | Invalid_priority of priority
  | Custom of { code : int; message : string }

exception Queue_error of error

val empty : 'a t
val is_empty : 'a t -> bool
val push : priority -> 'a -> 'a t -> 'a t
val pop : 'a t -> ('a * 'a t) option
val pop_exn : 'a t -> 'a * 'a t
(** @raise Queue_error [Empty] when the queue has no elements. *)

val of_list : ?compare:(priority -> priority -> int) -> (priority * 'a) list -> 'a t
val fold : f:('acc -> priority -> 'a -> 'acc) -> init:'acc -> 'a t -> 'acc
val to_seq : 'a t -> (priority * 'a) Seq.t
external unsafe_size : 'a t -> int = "caml_pq_size"

module type ORDERED = sig
  type t
  val compare : t -> t -> int
end

module Make (Ord : ORDERED) : sig
  type key = Ord.t
  type +'a t
  val add : key -> 'a -> 'a t -> 'a t
end

class ['a] mutable_queue : object
  method push : priority -> 'a -> unit
  method pop : 'a option
end
val pp : (Format.formatter -> 'a -> unit) -> Format.formatter -> 'a t -> unit [@@ocaml.deprecated "use Fmt"]
