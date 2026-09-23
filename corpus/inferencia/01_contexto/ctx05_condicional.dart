// R-CTX-05: os dois ramos de `?:` recebem o contexto; sem contexto o tipo é
// UP dos ramos.
void main(List<String> args) {
  bool b = args.isEmpty;
  List<num> a = b ? /*@*/[1] : /*@*/[2.0];
  var c = /*@*/b ? 1 : 2.0;
  var d = /*@*/b ? 1 : 'x';
  double e = b ? /*@*/1 : 2;
  print([a, c, d, e]);
}
