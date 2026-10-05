// D2 (docs/NATIVO-MAPAS-DE-PILHA-E-EXCECOES.md §7.3): muitos temporários e
// `phi`. Vinte textos vivos através de uma chamada que aloca, um `phi` de
// referência e um `double` desencaixotado vivo no laço.

String f(String s, int k) => '$s-$k';

void main() {
  var acumulado = 0;
  var x = 0.5;
  String? anterior;
  for (var r = 0; r < 50; r++) {
    final a0 = f('a', r);
    final a1 = f('b', r);
    final a2 = f('c', r);
    final a3 = f('d', r);
    final a4 = f('e', r);
    final a5 = f('f', r);
    final a6 = f('g', r);
    final a7 = f('h', r);
    final a8 = f('i', r);
    final a9 = f('j', r);
    final a10 = f('k', r);
    final a11 = f('l', r);
    final a12 = f('m', r);
    final a13 = f('n', r);
    final a14 = f('o', r);
    final a15 = f('p', r);
    final a16 = f('q', r);
    final a17 = f('r', r);
    final a18 = f('s', r);
    final a19 = f('t', r);
    final meio = f(a0 + a19, r);
    acumulado += a0.length + a1.length + a2.length + a3.length + a4.length;
    acumulado += a5.length + a6.length + a7.length + a8.length + a9.length;
    acumulado += a10.length + a11.length + a12.length + a13.length + a14.length;
    acumulado += a15.length + a16.length + a17.length + a18.length + a19.length;
    acumulado += meio.length;
    x = x * 2 % 7;
    anterior = r.isEven ? a5 : a7;
  }
  print('$acumulado $anterior $x');
}
