mixin A {
  abstract final int a;
}

class const B(this.a) with A {
  @override
  final int a;
}
