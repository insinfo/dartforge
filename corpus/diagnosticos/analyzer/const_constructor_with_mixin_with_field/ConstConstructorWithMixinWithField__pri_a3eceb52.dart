mixin A {
  var a;
  var b;
}

class const B() extends Object with A {}
//    ^^^^^
// [diag.constConstructorWithMixinWithFields] This constructor can't be declared 'const' because the mixins add the instance fields: 'A.a', 'A.b'.
