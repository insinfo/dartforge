// Regenerate the fixture with graphs 2.3.2 and a package configuration from
// corpus/builders after pub get. Existing components in the input are overwritten:
// dart --packages=<package_config.json> scc_oracle.dart <input.json>
import 'dart:convert';
import 'dart:io';
import 'package:graphs/graphs.dart';

void main(List<String> args) {
  final cases = jsonDecode(File(args[0]).readAsStringSync()) as List;
  for (final c in cases) {
    final edges =
        (c['edges'] as List).map((e) => (e as List).cast<int>()).toList();
    c['components'] = stronglyConnectedComponents<int>(
        (c['start'] as List).cast<int>(), (n) => edges[n]);
  }
  stdout.write(jsonEncode(cases));
}
