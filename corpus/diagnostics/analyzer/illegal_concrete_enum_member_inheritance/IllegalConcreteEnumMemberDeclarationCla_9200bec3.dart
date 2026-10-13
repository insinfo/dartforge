mixin M {
  int get hashCode;
}

abstract class B with M implements Enum {}
