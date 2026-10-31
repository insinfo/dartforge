// erro-de-compilacao
// The value of a `void` expression used as an argument (`use_of_void_result`):
// the CFE refuses the program; DartForge compiled it and passed `null`.
class Caixa {
  final List<String> itens = [];
}

void main() {
  final c = Caixa();
  final log = <Object?>[];
  log.add(c.itens.add('x'));
  print(log);
}
