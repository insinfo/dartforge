abstract class A {
  int get hashCode;
}

mixin M on A implements Enum {}
