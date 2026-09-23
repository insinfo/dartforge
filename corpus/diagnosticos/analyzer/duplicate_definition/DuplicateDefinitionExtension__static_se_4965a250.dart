extension E on int {
  static void set foo(_) {}
}

augment extension E {
  augment static void set foo(_);
}
