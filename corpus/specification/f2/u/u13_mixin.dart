mixin _NaoUsado {}
mixin _Usado {}
mixin _SoOn on Object {}
class A with _Usado {}
mixin B on _SoOn {}
