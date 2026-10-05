void f(int x) {
  switch (x) {
    case 0:
      print(0);
    case _:
      print(1);
    case 1:
      print(2);
    case 2:
    case 3:
      print(3);
    default:
      print(4);
  }
  print(5);
}
