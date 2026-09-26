// erro-de-compilacao
// Um local declarado duas vezes no mesmo escopo e um nome que não existe:
// o programa é recusado na compilação (o primeiro erro é o da linha 7).
void main() {
  final bytes = 1;
  print(bytes);
  final bytes = 2;
  print(bytes + naoExiste);
}
