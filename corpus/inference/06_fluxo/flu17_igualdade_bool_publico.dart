// R-FLU-17: `== true` não promove; propriedade pública não promove ("why
// not promoted"); `this` não promove.
class P {
  int? v;
  void m() {
    if (this is Q) print(/*@*/this);
  }
}

class Q extends P {}

void f(bool? b, P p) {
  if (b == true) print(/*@*/b);
  if (p.v != null) print(/*@*/p.v);
}

void main() => f(true, P());
