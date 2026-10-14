const a = 'A';
const b = 'B';
const joined = '$a$b$a$b$a$b$a$b$a$b$a$b$a$b$a$b';
const high = '\uD83D';
const low = '\uDE00';
const emoji = '$high$low';
const lone = '\uD800';
const floating = '${1.0}';
const nestedFloating = '<$floating>';
const environment = bool.fromEnvironment('missing', defaultValue: true);
const environmentText = '$environment';
int calls = 0;
String get observed { calls++; return 'runtime'; }
class User {
  String toString() { calls++; return 'user'; }
}
String parameterText(String n) => '<$n>';
String localOne() { const n = 'first'; return '<$n>'; }
String localTwo() { const n = 'second'; return '<$n>'; }
void units(String s) {
  print(s.length);
  for (var i = 0; i < s.length; i++) { print(s.codeUnitAt(i)); }
}
void main() {
  const local = 'local';
  print(joined);
  print(localOne());
  print(localTwo());
  print(parameterText('argument'));
  print('$local/$joined/${2 + 3}/${true}/${null}');
  units(emoji);
  units('$lone/lone');
  print(nestedFloating);
  print(environmentText);
  print('$observed/${User()}');
  print(calls);
}
