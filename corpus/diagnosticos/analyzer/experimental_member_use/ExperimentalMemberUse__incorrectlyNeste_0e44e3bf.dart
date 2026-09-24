class C {
  final String x;
  final bool y;

  const C({
//      ^
// [diag.finalNotInitializedConstructor1] All final variables must be initialized, but 'y' isn't.
    required this.x,
    {this.y = false}
//  ^
// [diag.missingIdentifier] Expected an identifier.
// [diag.expectedToken] Expected to find '}'.
  });
}

const z = C(x: '');
