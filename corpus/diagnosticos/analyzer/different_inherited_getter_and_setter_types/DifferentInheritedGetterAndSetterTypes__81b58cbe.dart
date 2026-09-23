class A<T, U> {
  void set foo(T _) {}
  U get foo => throw 0;
}

class C extends A<int, num> {
  var foo = 0;
//    ^^^
// [diag.differentInheritedGetterAndSetterTypes] Can't infer a type for 'foo' because the combined member signature of the getter has return type 'num', which is not the same as the parameter type 'int' of the combined member signature of the setter.
}
