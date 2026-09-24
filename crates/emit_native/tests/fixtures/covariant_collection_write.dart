void main() {
  final ints = <int>[1];
  final List<num> nums = ints;
  try {
    nums.add(1.5);
    print('list accepted');
  } catch (e) {
    print(e is TypeError);
  }
  print(ints);

  final entries = <String, int>{'a': 1};
  final Map<String, num> values = entries;
  try {
    values['b'] = 1.5;
    print('map accepted');
  } catch (e) {
    print(e is TypeError);
  }
  print(entries);

  final unique = <int>{1};
  final Set<num> elements = unique;
  try {
    elements.add(1.5);
    print('set accepted');
  } catch (e) {
    print(e is TypeError);
  }
  print(unique);
}
