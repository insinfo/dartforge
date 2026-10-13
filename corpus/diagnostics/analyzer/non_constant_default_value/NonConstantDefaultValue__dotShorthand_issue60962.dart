class A {
  const A();
}

void f([A a = .new()]) {}
//            ^^^^^^
// [diag.nonConstantDefaultValue] The default value of an optional parameter must be constant.
