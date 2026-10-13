class A {
  final int x, y;
  A(this.x, [this.y = 0]);
}

class C extends A {
  final int _;
//          ^
// [diag.unusedField] The value of the field '_' isn't used.

  C(this._, super._, [super._]);
}
