void f(Object x) {
  if (x case int _ when x.isEven) {
    x.isEven;
  } else {
    x.isEven;
  }
  if (x case String s when s.isEmpty) {
    print(s);
  } else {
    print(x);
  }
}
