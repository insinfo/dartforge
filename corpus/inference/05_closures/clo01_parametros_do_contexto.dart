// R-CLO-01: parâmetros sem tipo recebem o tipo do contexto; sem contexto,
// dynamic.
void main() {
  void Function(int) f = (x) {
    print(/*@*/x);
  };
  int Function(String) g = /*@*/(s) => s.length;
  var h = /*@*/(x) => x;
  Function k = /*@*/(x) => 1;
  Object o = /*@*/(x) => 1;
  var l = [1].map(/*@*/(x) => x);
  print([f, g, h, k, o, l]);
}
