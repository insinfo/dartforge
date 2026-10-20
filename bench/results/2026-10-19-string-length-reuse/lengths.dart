@pragma('vm:never-inline')
int observe(int n) => (n * 3) ^ 7;

@pragma('vm:never-inline')
int stable(String text) {
  var sum = 0;
  for (var i = 0; i < text.length; i++) {
    sum += observe(i) + text.length;
  }
  return sum;
}

@pragma('vm:never-inline')
int changing(String first, String second) {
  var text = first;
  var i = 0;
  var sum = 0;
  while (i < text.length) {
    sum += observe(i) + text.length;
    text = i.isEven ? second : first;
    i++;
  }
  return sum;
}

@pragma('vm:never-inline')
int nullable(String? text, bool enter) {
  if (!enter) return 123;
  return stable(text!);
}

void main() {
  var total = 0;
  for (var n = 0; n < 20; n++) {
    final latin = String.fromCharCodes(List.generate(n, (i) => 65 + i));
    final wide = String.fromCharCodes(List.generate(n + 1, (i) => 0x400 + i));
    total += stable(latin) + stable(wide);
    total += changing(latin, wide) + changing(wide, latin);
    total += nullable(null, false) + nullable(wide, true);
  }
  print(total);
}
