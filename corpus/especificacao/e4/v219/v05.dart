// @dart=2.19
f(x) { switch (x) { case 1 ?? 2: break; case void fun() {}: break; case assert(false): break; case ++x: break; } }
