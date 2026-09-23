// R-GEN-10: restrições vindas de tipos de função (parâmetros contravariantes:
// DOWN dos limites superiores).
T f<T>(void Function(T) a, void Function(T) b) => throw 0;
T g<T>(T Function() a, T Function() b) => a();
void main() {
  var p = /*@*/f((int x) {}, (num x) {});
  var r = /*@*/g(() => 1, () => 2.5);
  print([p, r]);
}
