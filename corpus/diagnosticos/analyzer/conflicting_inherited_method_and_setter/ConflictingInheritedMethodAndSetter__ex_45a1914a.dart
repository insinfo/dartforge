extension type Base(Object? it) {
  void foo() {}
}

extension type Left(Object? it) implements Base {}

extension type Right(Object? it) implements Base {}

extension type C(Object? it) implements Left, Right {}
