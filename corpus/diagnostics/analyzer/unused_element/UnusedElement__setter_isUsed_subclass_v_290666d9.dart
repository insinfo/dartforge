class A {
  set _value(int v) {}
}

extension E on A {
  set value(int v) => _value = v;
}

class B extends A {
  @override
  set _value(int v) {}
}


void main() {
  A().value = 1;
  B().value = 1;
}
