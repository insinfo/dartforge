void f(void Function()? a, int Function(int)? Function()? b,
    List<void Function()?> c) {
  int x = a;
  int y = b;
  int z = c;
  print([x, y, z]);
}
