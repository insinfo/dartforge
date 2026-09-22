// DateTime em UTC: construtor utc, toString/toIso8601String, add/subtract, difference, comparação, campos, parse e epoch.
void main() {
  var d = DateTime.utc(2024, 2, 29, 13, 45, 30, 123);
  print(d);
  print(d.toIso8601String());
  print(d.isUtc);
  print(d.year);
  print(d.month);
  print(d.day);
  print(d.hour);
  print(d.minute);
  print(d.second);
  print(d.millisecond);
  print(d.weekday);
  print(d.weekday == DateTime.thursday);
  print(DateTime.utc(2024, 1, 1).weekday);
  print(DateTime.utc(2024, 1, 7).weekday == DateTime.sunday);
  print(DateTime.utc(2024));
  print(DateTime.utc(2024, 12));
  print(DateTime.utc(1999, 12, 31, 23, 59, 59));
  print(DateTime.utc(2024, 13, 1));
  print(DateTime.utc(2024, 1, 32));
  print(DateTime.utc(2024, 3, 0));
  print(DateTime.utc(2023, 2, 29));
  print(DateTime.utc(2024, 1, 1, 25));
  print(DateTime.utc(2024, 1, 1, 0, 0, 0, 1000));
  print(DateTime.utc(1970));
  print(DateTime.utc(1969, 12, 31, 23, 59, 59));
  print(DateTime.utc(1, 1, 1));
  print(DateTime.utc(9999, 12, 31));

  var d2 = d.add(Duration(days: 1, hours: 12));
  print(d2);
  print(d2.isUtc);
  print(d.subtract(Duration(days: 60)));
  print(d.add(Duration(milliseconds: 877)));
  print(d.add(Duration(days: 366)));
  print(d.add(Duration.zero) == d);
  print(d2.difference(d));
  print(d.difference(d2));
  print(d2.difference(d).inHours);
  print(DateTime.utc(2024, 3, 1).difference(DateTime.utc(2024, 2, 1)).inDays);
  print(DateTime.utc(2023, 3, 1).difference(DateTime.utc(2023, 2, 1)).inDays);
  print(DateTime.utc(2025).difference(DateTime.utc(2024)).inDays);
  print(d.compareTo(d2));
  print(d2.compareTo(d));
  print(d.compareTo(DateTime.utc(2024, 2, 29, 13, 45, 30, 123)));
  print(d.isBefore(d2));
  print(d.isAfter(d2));
  print(d.isAtSameMomentAs(DateTime.utc(2024, 2, 29, 13, 45, 30, 123)));
  print(d == DateTime.utc(2024, 2, 29, 13, 45, 30, 123));
  print(d == d2);
  print(d.hashCode == DateTime.utc(2024, 2, 29, 13, 45, 30, 123).hashCode);

  var p = DateTime.parse('2024-02-29T13:45:30Z');
  print(p);
  print(p.isUtc);
  print(p.toIso8601String());
  print(DateTime.parse('2024-02-29T13:45:30.5Z'));
  print(DateTime.parse('2024-02-29 13:45:30Z'));
  print(DateTime.parse('2024-02-29T13:45Z'));
  print(DateTime.parse('20240229T134530Z'));
  print(DateTime.parse('2024-02-29T13:45:30+02:00'));
  print(DateTime.parse('2024-02-29T13:45:30-05:30'));
  print(DateTime.parse('2024-02-29T13:45:30+00:00').isUtc);
  print(DateTime.parse('-0001-01-01T00:00:00Z'));
  print(DateTime.parse('+10000-01-01T00:00:00Z'));
  print(DateTime.tryParse('nada'));
  print(DateTime.tryParse('2024-02-29T00:00:00Z'));
  try {
    DateTime.parse('2024-13-45');
  } catch (e) {
    print('lançou ${e is FormatException}');
  }
  print(p == d.subtract(Duration(milliseconds: 123)));

  print(DateTime.utc(1970).millisecondsSinceEpoch);
  print(DateTime.utc(1970, 1, 2).millisecondsSinceEpoch);
  print(DateTime.utc(2024, 2, 29, 13, 45, 30, 123).millisecondsSinceEpoch);
  print(DateTime.utc(1969, 12, 31, 23, 59, 59).millisecondsSinceEpoch);
  print(DateTime.utc(2000).microsecondsSinceEpoch);
  print(DateTime.fromMillisecondsSinceEpoch(0, isUtc: true));
  print(DateTime.fromMillisecondsSinceEpoch(1709214330123, isUtc: true));
  print(DateTime.fromMillisecondsSinceEpoch(-1000, isUtc: true));
  print(DateTime.fromMillisecondsSinceEpoch(86400000 * 365, isUtc: true));
  print(DateTime.fromMicrosecondsSinceEpoch(946684800000000, isUtc: true));
  print(DateTime.fromMillisecondsSinceEpoch(1709214330123, isUtc: true) ==
      DateTime.utc(2024, 2, 29, 13, 45, 30, 123));
  print(d.toUtc() == d);
  print(d.toUtc().isUtc);
  print(d.timeZoneName);
  print(d.timeZoneOffset);
  print(DateTime.utc(2024, 2, 29).copyWith(year: 2023, month: 3));
  print(DateTime.utc(2024, 2, 29).copyWith(hour: 23, isUtc: true));
  var datas = [DateTime.utc(2024, 3), DateTime.utc(2023), DateTime.utc(2024, 1, 15)];
  datas.sort();
  print(datas);
  print(datas.map((x) => x.year).toList());
  print(DateTime.monday);
  print(DateTime.sunday);
  print(DateTime.daysPerWeek);
  print(DateTime.monthsPerYear);
  print(DateTime.december);
  var inicio = DateTime.utc(2024, 1, 31);
  for (var i = 1; i <= 3; i++) {
    print(DateTime.utc(inicio.year, inicio.month + i, inicio.day));
  }
  print(DateTime.utc(2024, 2, 29).add(Duration(days: 365)));
  print(DateTime.utc(2024, 2, 29).add(Duration(days: 365)).weekday);
}
