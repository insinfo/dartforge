void f(Object? o, int i) {
  switch (o) {
    case int a:
    case num a:
      String s = a;
    case String _ || Null _:
      break;
    case var z:
      String s = z;
  }
  switch (i) {
    case int() && var p:
      print(p);
    case var q:
      print(q);
  }
  print(9);
}
