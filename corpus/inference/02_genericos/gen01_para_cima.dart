// R-GEN-01: sem contexto, os argumentos de tipo vêm dos argumentos (fase
// para cima): T :> tipo do argumento; escolhe-se o limite inferior.
T id<T>(T x) => x;
void main() {
  var a = /*@*/id(1);
  var b = /*@*/id([1, 2.5]);
  var c = /*@*/id(null);
  print([a, b, c]);
}
