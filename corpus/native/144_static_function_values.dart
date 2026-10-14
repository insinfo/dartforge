// Static function fields/getters read the callee before argument effects.
String trace = '';
int oldCall(int value, {int scale = 1}) { trace += 'O'; return value * scale; }
int newCall(int value, {int scale = 1}) { trace += 'N'; return -value * scale; }
T identity<T>(T value, {required String tag}) { trace += tag; return value; }
class ErrorWidget {
  static int Function(int, {int scale}) builder = oldCall;
  static int Function(int, {int scale}) get getter { trace += 'G'; return builder; }
  static T Function<T>(T, {required String tag}) generic = identity;
  static int implicit(int value) => builder(value, scale: 2);
}
typedef Alias = ErrorWidget;
int argument() { trace += 'A'; ErrorWidget.builder = newCall; return 7; }
int named() { trace += 'S'; return 3; }
int Function(int, {int scale}) get top { trace += 'T'; return oldCall; }
void main() {
  print(ErrorWidget.builder(argument(), scale: named()));
  print(trace);
  trace = '';
  ErrorWidget.builder = oldCall;
  print(Alias.getter(argument(), scale: named()));
  print(trace);
  trace = '';
  print(top(argument(), scale: named()));
  print(trace);
  trace = '';
  print(ErrorWidget.generic<int>(9, tag: 'I'));
  print(ErrorWidget.generic<String>('typed', tag: 'S'));
  print(trace);
  trace = '';
  ErrorWidget.builder = oldCall;
  print(ErrorWidget.implicit(4));
  print(trace);
}
