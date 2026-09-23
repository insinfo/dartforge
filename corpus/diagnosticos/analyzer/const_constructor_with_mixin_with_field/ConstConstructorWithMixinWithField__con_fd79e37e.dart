mixin A {
  abstract int a;
}

class B with A {
  @override
  int a;
  const B(this.a);
//      ^
// [diag.constConstructorWithMixinWithField] This constructor can't be declared 'const' because a mixin adds the instance field: 'A.a'.
}
