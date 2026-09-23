typedef Fn<T> = void Function(T);

void bar() {
  Fn<int>.foo;
//        ^^^
// [diag.undefinedGetterOnFunctionType] The getter 'foo' isn't defined for the 'Fn' function type.
}

extension E on Type {
  int get foo => 1;
}
