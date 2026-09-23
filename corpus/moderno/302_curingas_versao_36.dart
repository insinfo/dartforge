// @dart=3.6
// requer-dart: 3.13
// A mesma sintaxe numa biblioteca 3.6, no SDK 3.13: `_` volta a ligar nome
// (a versão é da biblioteca, não do SDK).
void main() {
  var _ = 1;
  print(_ + 1);
  void g(int _, int __) => print(_);
  g(9, 0);
  [5].forEach((_) => print(_ * 2));
  try {
    throw 'e';
  } catch (_) {
    print('pegou $_');
  }
}
