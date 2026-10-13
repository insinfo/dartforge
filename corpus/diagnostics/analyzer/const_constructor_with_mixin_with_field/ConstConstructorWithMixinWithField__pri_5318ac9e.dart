mixin A {
  abstract int a;
}

class const B(this.a) with A {
//    ^^^^^
// [diag.constConstructorWithMixinWithField] This constructor can't be declared 'const' because a mixin adds the instance field: 'A.a'.
  @override
  final int a;
  set a(int x) {}
}
