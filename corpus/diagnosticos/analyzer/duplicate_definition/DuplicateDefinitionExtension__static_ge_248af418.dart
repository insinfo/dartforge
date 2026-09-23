extension E on int {
  static int get foo => 0;
}

augment extension E {
  augment static int get foo;
}
