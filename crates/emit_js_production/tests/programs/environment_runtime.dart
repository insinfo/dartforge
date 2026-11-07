dynamic observed(String label, dynamic value) {
  print(label);
  return value;
}

void main() {
  try {
    String.fromEnvironment(defaultValue: observed('default first', 42),
        observed('name later', 'x'));
  } catch (error) {
    print(error is TypeError);
    print(error is UnsupportedError);
  }
  try {
    String.fromEnvironment(observed('name first', 'x'),
        defaultValue: observed('default later', 42));
  } catch (error) {
    print(error is TypeError);
    print(error is UnsupportedError);
  }
  try {
    bool.fromEnvironment(defaultValue: observed('bool default', 42),
        observed('bool name', 'x'));
  } catch (error) {
    print(error is TypeError);
    print(error is UnsupportedError);
  }
  try {
    String.fromEnvironment(observed('invalid name first', 42),
        defaultValue: observed('default must not run', 'fallback'));
  } catch (error) {
    print(error is TypeError);
    print(error is UnsupportedError);
  }
  try {
    bool.hasEnvironment(observed('presence', 'x'));
  } catch (error) {
    print(error is UnsupportedError);
  }
}
