mixin A {
  abstract final int a;
}

class B with A {
  @override
  final int a;
  const B(this.a);
}
