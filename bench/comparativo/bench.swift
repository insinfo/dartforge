// Comparativo entre linguagens (RELATORIO-COMPARATIVO-NATIVO.md): os mesmos
// quatro núcleos em cada uma; `scripts/comparar-linguagens.py` compila e mede.
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
medir("arvores") { arvores(14) }
medir("numerico") { numerico(30000000) }
medir("textos") { textos(1000000) }
