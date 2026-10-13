abstract class A {}

mixin M1 implements A {}

mixin M2 on A {}

enum E with M1, M2 {
  v
}
