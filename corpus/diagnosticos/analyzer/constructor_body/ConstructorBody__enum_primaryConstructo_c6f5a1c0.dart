enum E(int x) {
  v(0);
}
augment enum E {;
  augment E(int x) : assert(true);
}
