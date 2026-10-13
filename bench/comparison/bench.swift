// Comparison between languages (RELATORIO-COMPARATIVO-NATIVO.md): the same
// five kernels in each one; `scripts/compare-languages.py` compiles and measures.
import Foundation

final class No {
    let esq: No?
    let dir: No?
    init(_ e: No?, _ d: No?) { esq = e; dir = d }
    func conta() -> Int { 1 + (esq?.conta() ?? 0) + (dir?.conta() ?? 0) }
}

func arvore(_ d: Int) -> No { d == 0 ? No(nil, nil) : No(arvore(d - 1), arvore(d - 1)) }

func arvores(_ maxD: Int) -> Int {
    let longa = arvore(maxD)
    var total = 0
    var d = 4
    while d <= maxD {
        let it = 1 << (maxD - d + 4)
        for _ in 0..<it { total += arvore(d).conta() }
        d += 2
    }
    return total + longa.conta()
}

func fib(_ n: Int) -> Int { n < 2 ? n : fib(n - 1) + fib(n - 2) }

func numerico(_ n: Int) -> Int {
    var s = 0
    for i in 0..<n { s = (s + i * i) % 1000003 }
    return s
}

func textos(_ n: Int) -> Int {
    var m = [String: Int]()
    for i in 0..<n { let k = "k" + String(i % 5000); m[k, default: 0] += i }
    var s = 0
    for (_, v) in m { s += v % 1000 }
    return s
}

func json(_ n: Int) -> Int {
    var dados: [[String: Any]] = []
    dados.reserveCapacity(n)
    for i in 0..<n {
        dados.append(["id": i, "name": "item\(i)", "tags": ["a\(i % 7)", "b\(i % 13)"], "active": i % 2 == 0, "level": i % 100])
    }
    let texto = try! JSONSerialization.data(withJSONObject: dados)
    let lidos = try! JSONSerialization.jsonObject(with: texto) as! [[String: Any]]
    var s = 0
    for m in lidos {
        s += (m["id"] as! Int) + (m["name"] as! String).count + (m["tags"] as! [Any]).count + ((m["active"] as! Bool) ? 1 : 0) + (m["level"] as! Int)
    }
    return s
}

func medir(_ nome: String, _ f: () -> Int) {
    var r = 0
    var t: [String] = []
    for _ in 0..<4 {
        let t0 = DispatchTime.now().uptimeNanoseconds
        r = f()
        t.append(String((DispatchTime.now().uptimeNanoseconds - t0) / 1_000_000))
    }
    print("\(nome): \(t.joined(separator: " ")) ms | \(r)")
}

medir("fib") { fib(32) }
medir("trees") { arvores(14) }
medir("numeric") { numerico(30000000) }
medir("strings") { textos(1000000) }
medir("json") { json(50000) }
