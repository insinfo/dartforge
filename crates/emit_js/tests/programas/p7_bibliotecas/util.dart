library util;
part "util_part.dart";
int twice(int x) => x * 2;
class Counter { int n = 0; void inc() => n++; }
const answer = 42;
String get greeting => "hi";
enum Mode { fast, slow }
extension IntX on int { int get sq => this * this; String rep(int k) => toString() * k; static int zero() => 0; }
typedef IntFn = int Function(int);
typedef Pair = (int, String);
