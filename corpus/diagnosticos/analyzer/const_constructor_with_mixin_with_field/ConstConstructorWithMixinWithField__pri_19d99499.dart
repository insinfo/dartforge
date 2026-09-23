mixin A {
  var a;
}

class const B() extends Object with A {}
//    ^^^^^
// [diag.constConstructorWithMixinWithField] This constructor can't be declared 'const' because a mixin adds the instance field: 'A.a'.
