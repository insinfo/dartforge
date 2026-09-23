mixin M1 {}
mixin M2 on M1 {}

enum E with M1 {
  v
}
augment enum E with M2 {}
