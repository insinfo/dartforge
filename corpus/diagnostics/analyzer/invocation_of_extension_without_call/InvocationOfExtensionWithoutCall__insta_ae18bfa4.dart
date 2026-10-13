extension E on Object {}
f() {
  E(0)();
//^^^^
// [diag.invocationOfExtensionWithoutCall] The extension 'E' doesn't define a 'call' method so the override can't be used in an invocation.
}
