class _A {
  _A();
  _A.n();
}
class _B extends _A {
  _B() : super();
  _B.n() : super.n();
}
class _C {
  _C.new();
  _C.n();
}
void main() {
  _B();
  _B.n();
  _C.n();
}
