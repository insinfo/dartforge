// R-FLU-12: promoção de campo final privado (3.2); campo público ou não
// final não promove.
class C {
  final int? _x;
  final int? pub;
  int? _mutavel;
  C(this._x, this.pub);
  void m(C outro) {
    if (_x != null) print(/*@*/_x);
    if (this._x != null) print(/*@*/this._x);
    if (outro._x != null) print(/*@*/outro._x);
    if (pub != null) print(/*@*/pub);
    if (_mutavel != null) print(/*@*/_mutavel);
  }
}

void main() => C(1, 2).m(C(3, 4));
