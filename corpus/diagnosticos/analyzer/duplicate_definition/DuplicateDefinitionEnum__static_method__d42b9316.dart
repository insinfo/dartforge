enum E {
  v;
  static void foo() {}
}

augment enum E {;
  augment static void foo();
}
