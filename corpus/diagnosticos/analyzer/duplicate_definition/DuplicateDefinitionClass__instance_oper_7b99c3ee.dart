class A {
  int operator +(int _) => 0;
}

augment class A {
  augment int operator +(int _);
}
