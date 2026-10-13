enum E {
  v;
  static int get foo => 0;
}

augment enum E {;
  augment static int get foo;
}
