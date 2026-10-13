mixin A {
  var a;
  var b;
}

class B extends Object with A {
  const B();
//      ^
// [diag.constConstructorWithMixinWithFields] This constructor can't be declared 'const' because the mixins add the instance fields: 'A.a', 'A.b'.
}
