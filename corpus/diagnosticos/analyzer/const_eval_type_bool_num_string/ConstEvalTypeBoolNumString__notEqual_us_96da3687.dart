// %before-language-feature: patterns
class A {
  const A();
}

const a = A();
const b = a != 0;
//        ^^^^^^
// [diag.constEvalTypeBoolNumString] In constant expressions, operands of this operator must be of type 'bool', 'num', 'String' or 'null'.
