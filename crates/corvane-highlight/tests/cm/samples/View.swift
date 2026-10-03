//
//  View.swift — a SwiftUI view with ünïcödé comments: 日本語
//

import SwiftUI
import Foundation

/* Block comment
   /* nested block comment */
   still a comment */

@MainActor
@available(iOS 17, *)
@propertyWrapper(`escaped`)
@$0
public final class ProfileViewModel: ObservableObject, @unchecked Sendable {
    @Published var name: String = "Ada Lovelace"
    @Published private(set) var age: Int = 36
    private let formatter: NumberFormatter? = nil
    lazy var scores: [Double] = [1.5, -2.25, 3e10, 4.2E-3, 1_000_000.000_1]
    weak var delegate: AnyObject?
    unowned let owner: Owner
    static let shared = ProfileViewModel(owner: Owner())

    let binary = 0b1010_1010
    let octal = 0o755
    let hex = 0xFF_EC
    let hexFloat = 0x1.8p3
    let hexExp = 0xAp-2
    let negative = -42
    let range = 0..<10
    let closed = 1...5
    let tuple = (1, "two", 3.0)

    init(owner: Owner) {
        self.owner = owner
        super.init()
    }

    deinit {
        print("bye")
    }

    func greeting(for person: String, times count: Int = 1) -> String {
        let repeated = String(repeating: "Hi \(person)! ", count: count)
        return "\(repeated) You are \(age + 1) next year, \(formatter?.string(from: NSNumber(value: age)) ?? "n/a")."
    }

    func nested() -> String {
        "outer \(items.map { "inner \($0.name) (\($0.id))" }.joined(separator: ", ")) done"
    }

    func escapes() -> [String] {
        ["tab\tnewline\n", "quote \" backslash \\", "unicode \u{1F600} é", 'single', ""]
    }

    func multiline() -> String {
        let text = """
            Dear \(name),
              "quoted" text and \(age) years
            with a triple quote below
            """
        return text
    }

    mutating func update<T: Equatable>(_ value: inout T, to newValue: T) where T: Hashable {
        guard value != newValue else { return }
        value = newValue
    }

    func load() async throws -> Data {
        let (data, _) = try await URLSession.shared.data(from: URL(string: "https://example.com")!)
        return data
    }

    func closures() {
        let sorted = scores.sorted { $0 > $1 }
        let mapped = scores.map { value -> Int in Int(value) * 2 }
        let filtered = scores.filter({ (x: Double) -> Bool in return x >= 0 && x <= 100 || !x.isNaN })
        _ = sorted.reduce(0, +)
        _ = mapped.compactMap { $0 % 3 == 0 ? $0 : nil }
        _ = filtered.first?.rounded() ?? 0
        let `class` = "escaped identifier"
        let x = y.`default`
        var total = 0; total += 1; total -= 2; total *= 3; total /= 4
        total = total << 1 >> 2 & 0xF | 0x1 ^ ~total
        _ = [1: "one", 2: "two"][1]!
    }

    subscript(index: Int) -> Double {
        get { scores[index] }
        set { scores[index] = newValue }
    }

    var computed: Int {
        willSet { print(newValue) }
        didSet { print(oldValue) }
    }
}

protocol Shape: AnyObject {
    associatedtype Unit
    var area: Double { get }
    func scaled(by factor: Double) -> Self
}

extension Array where Element == Int {
    func sum() -> Int { reduce(0, +) }
}

enum Direction: String, CaseIterable {
    case north = "N", south = "S"
    case east, west

    indirect case nested(Direction)
}

struct Point<T: Numeric>: Hashable {
    var x: T
    var y: T
}

typealias Callback = (Result<Int, Error>) -> Void

precedencegroup PowerPrecedence {
    higherThan: MultiplicationPrecedence
    associativity: right
}

infix operator ** : PowerPrecedence

actor Counter {
    private var value = 0
    nonisolated let id = UUID()
    func increment() -> Int {
        value += 1
        return value
    }
}

#if DEBUG
let mode = "debug"
#elseif os(macOS)
let mode = "mac"
#else
let mode = "release"
#endif

#warning("check this")
let file = #file
let line = __LINE__
let selector = #selector(ProfileViewModel.load)

for (index, item) in items.enumerated() where index % 2 == 0 {
    switch item {
    case let .some(value) where value > 0:
        fallthrough
    case .none:
        break
    default:
        continue
    }
}

repeat {
    defer { cleanup() }
    do {
        try risky()
    } catch let error as NSError {
        throw error
    } catch {
        print(error)
    }
} while false

let optional: Int? = nil
if let unwrapped = optional, case .some(let v) = optional {
    print(unwrapped, v, true, false, _ = 1)
}
let dollar = $0 + $12
let broken = "value \(compute(1,
    2)) tail" + "\(a..<b) \(c...) \((d)) \("str \(e)")"
let esc = "\\(not interpolated) \\\(interpolated)"
let unterminated = "this string never ends
let afterUnterminated = 1
let bad = `unclosed
let weird = 1 @ 2 \ 3 `
/* unterminated comment at the end
continues here
