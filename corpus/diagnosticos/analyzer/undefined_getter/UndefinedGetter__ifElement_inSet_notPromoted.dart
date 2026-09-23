f(int x) {
  return {if (x is String) x.length};
//                           ^^^^^^
// [diag.undefinedGetter] The getter 'length' isn't defined for the type 'int'.
}
