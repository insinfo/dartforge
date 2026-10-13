// R-GEN-05: inferência horizontal (2.18): argumentos-função cujos parâmetros
// dependem de T são adiados até T ser fixado pelos outros argumentos.
U aplica<T, U>(T x, U Function(T) f) => f(x);
U aplicaAntes<T, U>(U Function(T) f, T x) => f(x);
void main() {
  var r = /*@*/aplica(1, (v) => (/*@*/v).isEven);
  var s = /*@*/aplicaAntes((v) => (/*@*/v).toDouble(), 1);
  var l = [1, 2, 3];
  var soma = /*@*/l.fold(0, (a, b) => /*@*/a + b);
  var m = /*@*/l.map((x) => x.toString()).toList();
  print([r, s, soma, m]);
}
