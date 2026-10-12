// `a = aa = x` com locais `int?`: o valor da atribuição interna (o `int`)
// passa por caixa e saída da caixa duas vezes; a simplificação da HIR
// trocava `Unbox(Box(x))` por `x` sem seguir a cadeia de trocas da mesma
// volta, e um uso ficava apontando para um valor removido ("use of
// undefined value" no LLVM). É o `processBlock` do RIPEMD-128 do
// pointycastle, que o new_sali/backend usa (docs/NATIVO-PROJETOS-REAIS.md,
// C10).

class Resumo {
  final List<int> estado = [1, 2, 3, 4];
  int f(int a, int b, int c, int d) => a + b * 2 + c * 3 + d * 4;

  void processar() {
    int? a, aa;
    int? b, bb;
    int? c, cc;
    int? d, dd;
    a = aa = estado[0];
    b = bb = estado[1];
    c = cc = estado[2];
    d = dd = estado[3];
    a = f(a, b, c, d);
    d = f(d, a, b, c);
    c = f(c, d, a, b);
    b = f(b, c, d, a);
    print('$a $b $c $d $aa $bb $cc $dd');
    double? x, xx;
    x = xx = 1.5;
    bool? p, pp;
    p = pp = estado.length == 4;
    print('${x + xx} ${p && pp}');
  }
}

void main() => Resumo().processar();
