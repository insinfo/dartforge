int x = 0;
void f() {
  x;
  var x = 1;
  {
    y;
  }
  var y = 2;
  z();
  void z() {}
}
void g() {
  w;
  {
    var w = 0;
  }
  w;
}
