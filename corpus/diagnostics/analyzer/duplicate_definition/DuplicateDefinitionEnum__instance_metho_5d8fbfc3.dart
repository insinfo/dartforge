enum E {
  v;
  void foo() {}
}

augment enum E {;
  augment void foo();
}
