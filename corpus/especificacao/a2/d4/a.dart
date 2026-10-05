import 'lib.dart' deferred as p;
import 'lib.dart' deferred as q hide X;
import 'lib.dart' deferred as r show K;
void f() { p.loadLibrary(); q.loadLibrary(); r.loadLibrary(); }
