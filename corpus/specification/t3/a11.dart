void f(List<int> l, Map<String, int> m, Object o) {
  switch (l) {
    case [...]:
      print(1);
    case []:
      print(2);
  }
  switch (l) {
    case [_]:
      print(1);
    case [..., _]:
      print(2);
    case [...var r]:
      print(r);
    case _:
      print(4);
  }
  switch (m) {
    case {'a': _}:
      print(1);
    case Map():
      print(2);
    case _:
      print(3);
  }
  if (o case [var a, ...]) {
    String s = o;
    String t = a;
  }
  if (o case {'k': var v}) {
    String s = o;
    String t = v;
  }
}
