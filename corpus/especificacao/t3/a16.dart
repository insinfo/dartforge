void f(int x, bool c) {
  switch (x) {
    case _ when c:
      print(1);
    case _ when true:
      print(2);
    case 1:
      print(3);
  }
  switch (x) {
    case _ when c:
      print(1);
    case int _:
      print(2);
    case 1:
      print(3);
  }
  var r = switch (x) { _ when c => 1, var y => y, 3 => 4 };
}
