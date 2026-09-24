import 'dart:async';

class A<T> {}

class B extends A<FutureOr<Object>> {}

class C extends B implements A<Object> {}
