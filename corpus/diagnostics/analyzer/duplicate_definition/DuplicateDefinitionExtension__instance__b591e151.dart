extension E on int {
  int get foo => 0;
}

augment extension E {
  augment int get foo;
}
