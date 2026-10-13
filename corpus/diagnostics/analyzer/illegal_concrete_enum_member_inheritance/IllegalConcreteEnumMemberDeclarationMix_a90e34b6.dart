class A {
  bool operator ==(Object other) => false;
}

mixin M on A implements Enum {}
