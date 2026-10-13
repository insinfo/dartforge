void take(int i) {}
void f() {
  take(und);
  take(und.x);
  int a = und;
  und();
  und.m();
  var r = und + 1;
  r.foo;
  int b = r;
  take(und ?? 1);
  print([a, b]);
}
