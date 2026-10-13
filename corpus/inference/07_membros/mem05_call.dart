// R-MEM-05: objeto com `call`: invocação e tear-off implícito de `call`.
class F {
  int call(String s) => s.length;
}

void main() {
  var f = F();
  var a = /*@*/f('a');
  int Function(String) g = /*@*/f;
  var h = /*@*/f.call;
  var i = /*@*/f.call('b');
  Function j = /*@*/f;
  print([a, g, h, i, j]);
}
