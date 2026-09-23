// R-CLO-06: função local sem tipo de retorno escrito: inferido como em
// expressão de função; recursão.
void main() {
  f() => 1;
  g(int x) {
    return x * 2.5;
  }

  int fat(int n) => n <= 1 ? 1 : n * fat(n - 1);
  var a = /*@*/f();
  var b = /*@*/g(1);
  var c = /*@*/fat(3);
  print([a, b, c, /*@*/f, /*@*/g]);
}
