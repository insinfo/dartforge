extension E on int {
  void set foo(_) {}
}

augment extension E {
  augment void set foo(_);
}
