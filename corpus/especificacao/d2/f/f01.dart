class Eq { const Eq(); bool operator ==(Object o) => true; int get hashCode => 0; }
class H { const H(); int get hashCode => 0; }
int n = 0;
const bool t = true;
const u = bool.fromEnvironment('x');
var a = const [1, n, 'a'];
var b = const <int>[1, 'a', 1.5];
var c = const {1, n, 1, 2, 2};
var d = const {Eq(), H(), 1.5, 2.0};
var e = const {1: n, n: 2, 1: 3};
var f = const {Eq(): 1, H(): 2, 1.5: 3};
var g = const <int, String>{'a': 1};
var h = const [if (t) n else n, if (!t) n else 1];
var i = const [if (u) n else 1, if (n > 0) 1];
var j = const [for (var k = 0; k < 1; k++) k, for (var k in [1]) k];
var k = const [...[1, n], ...{2}, ...1, ...?null, ...n];
var l = const {...[1, 2], 1, ...{3}, ...[Eq()]};
var m = const {...{1: 2}, 1: 3, ...[1], ...?null};
var o = const {if (u) 1: n else n: 2};
var p = const {if (t) 1 else 2, 1};
