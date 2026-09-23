// R-GEN-02: o tipo de retorno casado com o contexto (fase para baixo);
// sem restrição nenhuma, T fica com o limite (dynamic).
List<T> vazio<T>() => <T>[];
void main() {
  List<num> a = /*@*/vazio();
  var b = /*@*/vazio();
  Iterable<String> c = /*@*/vazio();
  Object d = /*@*/vazio();
  print([a, b, c, d]);
}
