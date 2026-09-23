// @dart=3.6
// ignore_for_file: directives_ordering
// ignore_for_file: no_leading_underscores_for_library_prefixes
import 'package:build_runner_core/build_runner_core.dart' as _i1;
import 'package:sass_builder/sass_builder.dart' as _i2;
import 'package:build/build.dart' as _i3;
import 'package:build_resolvers/builder.dart' as _i4;
import 'dart:isolate' as _i5;
import 'package:build_runner/build_runner.dart' as _i6;
import 'dart:io' as _i7;

final _builders = <_i1.BuilderApplication>[
  _i1.apply(
    r'sass_builder:sass_builder',
    [_i2.sassBuilder],
    _i1.toDependentsOf(r'sass_builder'),
    hideOutput: true,
    defaultDevOptions:
        const _i3.BuilderOptions(<String, dynamic>{r'sourceMaps': true}),
    defaultReleaseOptions: const _i3.BuilderOptions(<String, dynamic>{
      r'outputStyle': r'compressed',
      r'sourceMaps': false,
    }),
    appliesBuilders: const [r'sass_builder:sass_source_cleanup'],
  ),
  _i1.apply(
    r'build_resolvers:transitive_digests',
    [_i4.transitiveDigestsBuilder],
    _i1.toAllPackages(),
    isOptional: true,
    hideOutput: true,
    appliesBuilders: const [r'build_resolvers:transitive_digest_cleanup'],
  ),
  _i1.applyPostProcess(
    r'build_resolvers:transitive_digest_cleanup',
    _i4.transitiveDigestCleanup,
  ),
  _i1.applyPostProcess(
    r'sass_builder:sass_source_cleanup',
    _i2.sassSourceCleanup,
    defaultReleaseOptions:
        const _i3.BuilderOptions(<String, dynamic>{r'enabled': true}),
  ),
];
void main(
  List<String> args, [
  _i5.SendPort? sendPort,
]) async {
  var result = await _i6.run(
    args,
    _builders,
  );
  sendPort?.send(result);
  _i7.exitCode = result;
}
