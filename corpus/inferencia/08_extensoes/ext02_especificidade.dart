// R-EXT-02: entre extensões aplicáveis vence a mais específica; override
// explícito escolhe; membro de instância vence extensão.
extension A on Iterable<int> {
  String get q => 'a';
}

extension B on List<int> {
  int get q => 1;
}

extension X on String {
  String get length => '';
}

void main() {
  print([/*@*/[1].q, /*@*/A([1]).q, /*@*/'a'.length, /*@*/X('a').length]);
}
