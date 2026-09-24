typedef Fn<T> = void Function(T);

void bar() {
  Fn<int>.foo();
//        ^^^
// [diag.undefinedMethodOnFunctionType] The method 'foo' isn't defined for the 'Fn' function type.
}

extension E on Type {
  void foo() {}
}
