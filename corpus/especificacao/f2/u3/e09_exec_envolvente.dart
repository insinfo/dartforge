void _viaLocal() { void g() { _viaLocal(); } g(); }
void _viaClosure() { var c = () => _viaClosure(); c(); }
int get _g => _g;
set _s(int v) { _s = v; }
class A {
  int _f = 0;
  void _m() { () { _m(); }(); }
  void _n() { void h() { _n(); } h(); }
  void inc() { _f = _f + 1; }
  int get _p => _p;
}
