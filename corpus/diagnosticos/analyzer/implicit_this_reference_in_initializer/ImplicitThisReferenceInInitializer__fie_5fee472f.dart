class A {
  Map foo = {
    'a': () {
      var v = 0; // (1)
      v;
    },
    'b': _foo // (2)
//       ^^^^
// [diag.implicitThisReferenceInInitializer] The instance member '_foo' can't be accessed in an initializer.
  };

  void _foo() {}
}
