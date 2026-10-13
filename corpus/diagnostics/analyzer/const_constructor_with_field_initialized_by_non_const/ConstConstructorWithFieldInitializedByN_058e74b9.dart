int e = 3;
mixin class MixinClassFactory {
  final int foo = e;
  const factory MixinClassFactory.x() = A;
}

mixin class A implements MixinClassFactory {
  @override
  final int foo = 0;
  const A();
}
