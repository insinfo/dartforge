enum E {
  v;
  void set foo(_) {}
}

augment enum E {;
  augment void set foo(_);
}
