enum E {
  v;
  static void set foo(_) {}
}

augment enum E {;
  augment static void set foo(_);
}
