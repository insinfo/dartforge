class A { final int x; const A.n(this.x); }
const a = A.n;
const b = A.n(1).x;
void f(Object o) {
  switch (o) {
    case A.n:
    case const (A.new):
  }
}
