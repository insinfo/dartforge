class A {
  bool operator ==(Object other) => false;
}

abstract class B implements A, Enum {}
