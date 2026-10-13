mixin A {
  var a;
}

class B extends Object with A {
  const new();
//      ^^^
// [diag.constConstructorWithMixinWithField] This constructor can't be declared 'const' because a mixin adds the instance field: 'A.a'.
}
