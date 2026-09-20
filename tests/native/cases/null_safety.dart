int? maybeInt(bool present) { if (present) { return 7; } return null; }
bool? maybeBool(bool present) { if (present) { return false; } return null; }
int fallback() { print(900); return 12; }
int? echoInt(int? value) { return value; }
bool? echoBool(bool? value) { return value; }
int promoted(int? value) {
  if (value == null) { return 20; }
  return value + 1;
}
void inspect(int? number, bool? flag) {
  print(number); print(flag);
  print(number == flag); print(number != flag);
  print(number == null); print(null == flag);
  print(number ?? fallback());
  print(flag ?? true);
  print(number != null && number > 0);
  print(number == null || number > 0);
  if (number != null) { print(number! + 2); }
  if (flag != null) { print(!flag!); }
}
int? implicitInt(bool yes) { if (yes) { return 40; } }
bool? implicitBool(bool yes) { if (yes) { return true; } }
int? once() { print(901); return 8; }
void main() {
  print(implicitInt(false)); print(implicitInt(true));
  print(implicitBool(false)); print(implicitBool(true));
  print(once()!);
  print(maybeInt(false) ?? echoInt(null));
  inspect(maybeInt(false), maybeBool(false));
  inspect(maybeInt(true), maybeBool(true));
  print(echoInt(null)); print(echoInt(0));
  print(echoBool(null)); print(echoBool(true));
  print(promoted(null)); print(promoted(4));
  int? number = maybeInt(true);
  number = null;
  print(number);
  number = 3;
  print(number + 1);
  bool? flag = maybeBool(true);
  flag = null;
  print(flag);
  flag = false;
  print(!flag);
  print(null ?? null);
  print(null == null);
  print(0 == false);
  print(maybeInt(true)!);
  print(maybeBool(true)!);
  print(maybeInt(false) ?? maybeInt(false) ?? 33);
}
