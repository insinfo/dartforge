// Test selection shared by the DartForge test planner and suite workers.
//
// This reproduces the per-suite filter of package:test's `Runner._loadSuites`
// (global name patterns, included and excluded tags, per-path selections and
// sharding) and its unknown-tag warning, from a JSON description the planner
// derives from the real `Configuration`.
import 'package:boolean_selector/boolean_selector.dart';
import 'package:test_api/src/backend/group.dart';
import 'package:test_api/src/backend/group_entry.dart';
import 'package:test_api/src/backend/suite.dart';
import 'package:test_api/src/backend/test.dart';
import 'package:test_core/src/runner/runner_suite.dart';

/// Serializes a name pattern: `-n` creates regular expressions, `-N` strings.
Object? encodePattern(Pattern pattern) {
  if (pattern is RegExp) {
    return {
      'regex': pattern.pattern,
      'multiLine': pattern.isMultiLine,
      'caseSensitive': pattern.isCaseSensitive,
      'unicode': pattern.isUnicode,
      'dotAll': pattern.isDotAll,
    };
  }
  return {'plain': pattern as String};
}

Pattern decodePattern(Object? value) {
  final map = value as Map;
  if (map.containsKey('plain')) return map['plain'] as String;
  return RegExp(map['regex'] as String,
      multiLine: map['multiLine'] as bool,
      caseSensitive: map['caseSensitive'] as bool,
      unicode: map['unicode'] as bool,
      dotAll: map['dotAll'] as bool);
}

/// `BooleanSelector.all`/`none` have no parseable spelling.
Object? encodeSelector(BooleanSelector selector) {
  if (selector == BooleanSelector.all) return {'all': true};
  if (selector == BooleanSelector.none) return {'none': true};
  return {'text': selector.toString()};
}

BooleanSelector decodeSelector(Object? value) {
  final map = value as Map;
  if (map['all'] == true) return BooleanSelector.all;
  if (map['none'] == true) return BooleanSelector.none;
  return BooleanSelector.parse(map['text'] as String);
}

class Selection {
  final List<Pattern> globalPatterns;
  final BooleanSelector includeTags;
  final BooleanSelector excludeTags;

  /// One entry per `TestSelection` of the suite's path; each holds its
  /// patterns, all of which must match.
  final List<List<Pattern>> selections;
  final int? totalShards;
  final int? shardIndex;

  Selection(this.globalPatterns, this.includeTags, this.excludeTags,
      this.selections, this.totalShards, this.shardIndex);

  factory Selection.fromJson(Map json) => Selection(
        [for (final p in json['globalPatterns'] as List) decodePattern(p)],
        decodeSelector(json['includeTags']),
        decodeSelector(json['excludeTags']),
        [
          for (final s in json['selections'] as List)
            [for (final p in s as List) decodePattern(p)]
        ],
        json['totalShards'] as int?,
        json['shardIndex'] as int?,
      );

  bool _matches(Test test) {
    if (!globalPatterns.every((pattern) => test.name.contains(pattern))) {
      return false;
    }
    if (!includeTags.evaluate(test.metadata.tags.contains)) return false;
    if (excludeTags.evaluate(test.metadata.tags.contains)) return false;
    return selections.any(
        (patterns) => patterns.every((pattern) => test.name.contains(pattern)));
  }

  /// `Runner._loadSuites`'s filter followed by `Runner._shardSuite`.
  RunnerSuite apply(RunnerSuite suite) {
    final filtered = suite.filter(_matches);
    if (totalShards == null) return filtered;
    final shardSize = filtered.group.testCount / totalShards!;
    final shardStart = (shardSize * shardIndex!).round();
    final shardEnd = (shardSize * (shardIndex! + 1)).round();
    var count = -1;
    return filtered.filter((test) {
      count++;
      return count >= shardStart && count < shardEnd;
    });
  }
}

/// `Runner._warnForUnknownTags`, or `null` when every tag is known.
String? unknownTagsWarning(Suite suite, Set<String> knownTags, bool color) {
  final unknownTags = <String, List<GroupEntry>>{};
  final currentTags = <String>{};
  void collect(GroupEntry entry) {
    final newTags = <String>{};
    for (final unknownTag in entry.metadata.tags.difference(knownTags)) {
      if (currentTags.contains(unknownTag)) continue;
      unknownTags.putIfAbsent(unknownTag, () => []).add(entry);
      newTags.add(unknownTag);
    }
    if (entry is! Group) return;
    currentTags.addAll(newTags);
    for (final child in entry.entries) {
      collect(child);
    }
    currentTags.removeAll(newTags);
  }

  collect(suite.group);
  if (unknownTags.isEmpty) return null;

  String describe(GroupEntry entry) {
    if (entry is Test) return 'the test "${entry.name}"';
    if (entry.name.isNotEmpty) return 'the group "${entry.name}"';
    return 'the suite itself';
  }

  final yellow = color ? '\u001b[33m' : '';
  final bold = color ? '\u001b[1m' : '';
  final noColor = color ? '\u001b[0m' : '';
  final buffer = StringBuffer()
    ..write('${yellow}Warning:$noColor ')
    ..write(unknownTags.length == 1 ? 'A tag was ' : 'Tags were ')
    ..write('used that ')
    ..write(unknownTags.length == 1 ? "wasn't " : "weren't ")
    ..writeln('specified in dart_test.yaml.');
  unknownTags.forEach((tag, entries) {
    buffer.write('  $bold$tag$noColor was used in');
    if (entries.length == 1) {
      buffer.writeln(' ${describe(entries.single)}');
      return;
    }
    buffer.write(':');
    for (final entry in entries) {
      buffer.write('\n    ${describe(entry)}');
    }
    buffer.writeln();
  });
  return buffer.toString();
}
