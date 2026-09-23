// R-GEN-03: várias restrições inferiores para o mesmo T se juntam por UP.
T escolha<T>(T a, T b) => a;
void main() {
  var x = /*@*/escolha(1, 2.5);
  var y = /*@*/escolha(1, 'a');
  var z = /*@*/escolha(null, 1);
  var w = /*@*/escolha(<int>[], <double>[]);
  print([x, y, z, w]);
}
