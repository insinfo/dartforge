void f(List<int> l) {
  int x;
  for (x in l) {}
  int y = 0;
  for (y in l) { print(y); }
  var s = 'a';
  var t = 'b';
  print('$s');
  assert(t == 'b');
}
