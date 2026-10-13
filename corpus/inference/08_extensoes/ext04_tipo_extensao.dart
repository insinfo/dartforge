// R-EXT-04: tipos de extensão (3.3): o tipo estático é o próprio tipo de
// extensão; membros da representação.
extension type Id(int v) {
  Id proximo() => Id(v + 1);
}

extension type Nome(String s) implements String {}

void main() {
  var i = /*@*/Id(1);
  print([/*@*/i.proximo(), /*@*/i.v, /*@*/Nome('a').length, /*@*/i as int]);
}
