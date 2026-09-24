String test(C c) => c.m()();
//                  ^^^^^^^
// [diag.missingRequiredArgument] The named parameter 'x' is required, but there's no corresponding argument.

typedef String F({required String x});

class C {
  F m() => ({required String x}) => throw '';
}
