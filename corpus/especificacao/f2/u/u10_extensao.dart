extension _Priv on int {
  int get naoUsado => 0;
  int get usado => 1;
  void metodo() {}
  int operator +(int o) => 0;
  static void est() {}
}
extension on String {
  void semNomeNaoUsado() {}
  void semNomeUsado() {}
  void _privado() {}
}
extension Pub on double {
  void publico() {}
  void _privNaoUsado() {}
  void _privUsado() {}
  static void _est() {}
}
void main() {
  print(1.usado);
  ''.semNomeUsado();
  1.5._privUsado();
}
