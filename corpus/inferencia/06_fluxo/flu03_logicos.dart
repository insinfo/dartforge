// R-FLU-03: `&&`, `||`, `!` combinam os modelos verdadeiro/falso.
void f(Object o, int? x) {
  if (o is int && (/*@*/o).isEven) {}
  if (o is! int || (/*@*/o).isEven) {}
  if (!(o is String)) return;
  print(/*@*/o);
  if (x == null || (/*@*/x).isEven) {}
  var t = x != null && o.isEmpty;
  if (t) print(/*@*/x);
}

void main() => f('a', 1);
