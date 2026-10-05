class A {
  A.x();
}
class A {
  A.y();
  factory A.z() = A.x;
  factory A.w() = A.y;
}
void f() {
  A.x();
  A.y();
  A.z();
}
