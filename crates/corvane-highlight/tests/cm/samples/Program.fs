// Program.fs — an order-processing service in F#.
/// XML doc comment: <summary>Entry point</summary>
namespace Shop.Orders

open System
open System.Collections.Generic
open System.Threading.Tasks

(* Classic ML comment
   (* nested *) still in the comment
*)

[<Measure>] type EUR
[<Measure>] type kg

type OrderId = OrderId of Guid

type LineItem =
    { Sku: string
      Quantity: int
      UnitPrice: decimal<EUR> }

type OrderStatus =
    | Pending
    | Paid of amount: decimal<EUR>
    | Shipped of trackingNo: string * DateTime
    | Cancelled

[<AbstractClass>]
type Repository<'T>() =
    abstract member Find: Guid -> 'T option
    abstract member Save: 'T -> unit
    default this.ToString() = "Repository"

type IClock =
    abstract Now: DateTime

type InMemoryRepo<'T>(key: 'T -> Guid) =
    inherit Repository<'T>()
    let store = Dictionary<Guid, 'T>()
    let mutable writes = 0
    member val Name = "memory" with get, set
    static member Create k = InMemoryRepo<_>(k)
    override _.Find id =
        match store.TryGetValue id with
        | true, v -> Some v
        | false, _ -> None
    override _.Save item =
        writes <- writes + 1
        store.[key item] <- item
    interface IDisposable with
        member _.Dispose() = store.Clear()

module Pricing =
    let private taxRate = 0.19m
    let internal round2 (x: decimal) = Math.Round(x, 2)
    let inline total (items: LineItem list) =
        items |> List.sumBy (fun i -> decimal i.Quantity * decimal i.UnitPrice)
    let withTax t = round2 (t * (1m + taxRate))
    let hex = 0xFF and bin = 0b1010 and oct = 0o17 and sci = 1.5e-3
    let big = 1_000_000L and f32 = 3.14f and byteV = 255uy
    let weird = 0b102 + 0xG

let describe status =
    match status with
    | Pending -> "pending"
    | Paid amt when amt > 100m<EUR> -> sprintf "paid (big): %M" (decimal amt)
    | Paid amt -> $"paid: {amt}"
    | Shipped (no, at) -> String.Format("shipped {0} at {1:u}", no, at)
    | Cancelled -> "cancelled"

let verbatim = @"C:\temp\orders\"
let triple = """He said "hi" and left"""
let escaped = "tab\t newline\n quote\" backslash\\"
let multiLine = "first
second"
let chars = [ 'a'; '\n'; '"' ]

let fetchAll (ids: Guid seq) =
    async {
        let! results = ids |> Seq.map (fun id -> async { return id }) |> Async.Parallel
        do! Async.Sleep 10
        use! handle = Async.OnCancel(fun () -> printfn "cancelled")
        for r in results do
            yield! [ r ]
        return! async { return results.Length }
    }

let rec fib n = if n < 2 then n else fib (n - 1) + fib (n - 2)
let isEven = function
    | n when n % 2 = 0 -> true
    | _ -> false

let tryParse (s: string) =
    try
        Some(Int32.Parse s)
    with
    | :? FormatException as ex -> None
    finally
        printfn "done"

let check x =
    if x > 0 then "positive"
    elif x < 0 then "negative"
    else "zero"

let upcasted = upcast (InMemoryRepo<int>(fun _ -> Guid.Empty)) : obj
let downcasted = downcast (box 1) : int
let nothing = null
let flags = not true && false || (1 <> 2) ^^^ 3 &&& 4 ||| 5
let seqs = Seq.empty, List.empty, Map.empty, Set.empty, Option.None
let piped = [1..10] |> List.filter isEven |> List.map ((*) 2) >> id <| 3
let ops = x :: xs @ ys := !cell -> y <- z
let ``weird name with spaces`` = 1
let grüße = "Grüße, 世界 😀"
	let tabbed = 1	// tab before comment
let unterminated = "no closing quote
still in the string" + "done"
[<EntryPoint>]
let main argv =
    let repo = new InMemoryRepo<LineItem>(fun _ -> Guid.NewGuid())
    for i in 0 .. 2 .. 10 do printfn "%d" i
    while false do ()
    raise (InvalidOperationException "never") |> ignore
    failwith "unreachable"
    0 // exit code
(* open comment at end of file
