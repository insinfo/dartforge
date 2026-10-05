void f(int? n) {
  switch (n) {
    case int _:
      break;
    case var z:
      String s = n;
      String t = z;
      z.foo;
      List<String> u = [z];
  }
  if (n case int _) {
  } else if (n case var w) {
    String t = w;
    List<String> u = [w];
  }
  var v = n is int ? 0 : n;
  List<String> u = [v];
}
