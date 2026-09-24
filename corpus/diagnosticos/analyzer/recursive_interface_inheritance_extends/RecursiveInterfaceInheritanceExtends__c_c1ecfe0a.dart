class C extends C {
//              ^
// [diag.recursiveInterfaceInheritanceExtends] 'C' can't extend itself.
  var foo = 0;
  bar();
}
