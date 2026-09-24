class A {
  A.named();
}

augment class A {
  A.named();
//  ^^^^^
// [diag.duplicateConstructorName] The constructor with name 'named' is already defined.
}
