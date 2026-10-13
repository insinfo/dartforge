// %before-language-feature: wildcard-variables
f() {
  for (var _ in [1,2,3]) {
    for (var __ in [4,5,6]) {
      // do something
    }
  }
}
