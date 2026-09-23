class A {
  void set foo(num _) {}
//         ^^^
// [context 1] The setter being overridden.
  int get foo => 0;
}

class C extends A {
  var foo = 0;
//    ^^^
// [diag.differentInheritedGetterAndSetterTypes] Can't infer a type for 'foo' because the combined member signature of the getter has return type 'int', which is not the same as the parameter type 'num' of the combined member signature of the setter.
// [diag.invalidOverrideSetter][context 1] The setter 'C.foo' ('void Function(int)') isn't a valid override of 'A.foo' ('void Function(num)').
}
