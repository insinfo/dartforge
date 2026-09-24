class A {
  abstract var foo;
}

augment class A {
  augment dynamic foo = 0;
}
