class C {
  int? x;
  C(int? _);
}

augment class C {
  augment C(this.x);
}
