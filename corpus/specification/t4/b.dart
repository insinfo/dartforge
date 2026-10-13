Xyz g() => 1;
Xyz h() { return 'a'; }
Xyz? v;
void f() {
  int a = g();
  String b = v;
  v = 3;
  g().foo;
  var w = g();
  int c = w;
  w.zzz;
  print([a, b, c]);
}
