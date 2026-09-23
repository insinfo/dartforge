enum E {
  v;
  int get foo => 0;
}

augment enum E {;
  augment int get foo;
}
