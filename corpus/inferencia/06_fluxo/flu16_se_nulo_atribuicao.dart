// R-FLU-16: `x ??= e` promove `x` para NonNull depois; `x ?? (throw ...)`.
void f(int? x, int? y) {
  x ??= 1;
  print(/*@*/x);
  var z = y ?? (throw 0);
  print([/*@*/z, /*@*/y]);
}

void main() => f(1, 2);
