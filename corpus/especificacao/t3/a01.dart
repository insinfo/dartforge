void f(bool? b) {
  switch (b) {
    case true:
      print(1);
    case false:
      print(2);
    case null:
      print(3);
    case null:
      print(4);
  }
}
