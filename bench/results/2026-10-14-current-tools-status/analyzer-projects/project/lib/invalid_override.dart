class A { int f() => 1; }
class B extends A { String f() => 'wrong'; }
// Audit fixture: analyzer-projects
