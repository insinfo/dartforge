class A {
  factory B.new() => throw 0;
//        ^
// [diag.invalidFactoryNameNotAClass] The name of a factory constructor must be the same as the name of the immediately enclosing class.
  A();
}
