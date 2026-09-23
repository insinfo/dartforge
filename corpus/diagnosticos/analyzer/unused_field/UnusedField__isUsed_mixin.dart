mixin M {
  int _f = 0;
}
class Bar with M {
  int g() => _f;
}
