mixin M {}

class A {
  A();
  factory A.named() = A;
}

class B = A with M;

void main() {
  B.named();
//  ^^^^^
// [diag.undefinedMethodOnTypeLiteral] The method 'named' isn't defined for the type 'B'.
}
