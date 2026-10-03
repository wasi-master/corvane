#r "nuget: FSharp.Data, 6.3.0"
#load "Helpers.fsx"
#if INTERACTIVE
printfn "running in FSI"
#endif

// Script.fsx — quick data exploration
open FSharp.Data

type Weather = CsvProvider<"weather.csv", Separators = ";">

let data = Weather.Load(__SOURCE_DIRECTORY__ + "/weather.csv")

let avg xs = Seq.averageBy float xs
let temps = [ for r in data.Rows -> r.Temperature ]
let hottest = temps |> List.max
let coldest = temps |> List.min

printfn "avg %.2f, max %d, min %d" (avg temps) hottest coldest

(* comment with a "string" inside and *) let after = 2
(*) not a comment opener: (*) is the multiplication operator section *)
let times = (*) 3 4

let interpolated = $"hottest was {hottest}°C, coldest {coldest}°C"
let verbatimInterp = $@"path\{hottest}"
let quote = <@ 1 + 2 @>
let typed = <@@ fun x -> x @@>
let struct' = struct (1, 2)
let measure = 12.5<kg> * 2.0<kg>
let arr = [| 1; 2; 3 |]
arr.[0] <- 10
let slice = arr.[1..]
let optional = Some 42 |> Option.map ((+) 1) |> Option.defaultValue 0
let x' = 1
let y'' = x' + 1
let nums = [ 0x1F; 0o17; 0b11; 1e3; 2.5E+2; 3.; 10_000 ]
let str = "ends with escaped quote \"
still string" // the comment
let emptyStr = ""
let backslashEnd = "a\\"
let mutable counter = 0
counter <- counter + 1
lazy (printfn "lazy") |> ignore
let result = query { for r in data.Rows do select r.Temperature }
let evt = new Event<int>()
evt.Publish.Add(fun v -> printfn "%d" v)
// trailing comment without newline