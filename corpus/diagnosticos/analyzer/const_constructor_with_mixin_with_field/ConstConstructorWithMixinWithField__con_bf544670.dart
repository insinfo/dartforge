mixin A {
  final a = 0;
}

class B extends Object with A {
  const B();
//      ^
// [diag.constConstructorWithMixinWithField] This constructor can't be declared 'const' because a mixin adds the instance field: 'A.a'.
}
