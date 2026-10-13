// R-GEN-12: inferência horizontal: todo literal de função é adiado e as
// fases seguem as dependências entre argumentos (não a ordem do texto).
T f<T>(T Function() a, T b) => b;
U g<T, U>(U Function(T) a, T Function() b) => throw 0;
void main() {
  var r = /*@*/f(() => [], <int>[]);
  var s = /*@*/g((x) => (/*@*/x).isEven, () => 1);
  var t = /*@*/f((() => 1), 2.5);
  print([r, s, t]);
}
