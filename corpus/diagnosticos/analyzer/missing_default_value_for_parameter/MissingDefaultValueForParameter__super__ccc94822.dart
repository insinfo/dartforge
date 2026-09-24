class A {
  final int x, y;
  A(this.x, [this.y = 0]);
}

class C extends A {
  final int c;
  C(this.c, super._, [super._]);
}
