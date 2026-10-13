abstract class Foo {
  factory Foo({required String thing}) = _Foo._;
  Foo._({required this.thing});

  final String thing;

  void bar();
}

mixin _$Foo on Foo {
  @override
  void bar() {}
}

class _Foo = Foo with _$Foo;
