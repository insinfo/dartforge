typedef Fn<T> = void Function(T);

void bar() {
  Fn<int>.foo = 7;
//        ^^^
// [diag.undefinedSetterOnFunctionType] The setter 'foo' isn't defined for the 'Fn' function type.
}

extension E on Type {
  set foo(int value) {}
}
