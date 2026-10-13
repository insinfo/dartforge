class A {
  void set foo(int _) {}
  num get foo => 0;
}

class C extends A {
  var foo = 0;
//    ^^^
// [diag.differentInheritedGetterAndSetterTypes] Can't infer a type for 'foo' because the combined member signature of the getter has return type 'num', which is not the same as the parameter type 'int' of the combined member signature of the setter.
}
