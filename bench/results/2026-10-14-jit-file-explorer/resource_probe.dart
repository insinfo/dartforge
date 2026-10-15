import 'dart:convert';
import 'dart:io';
import 'dart:isolate';
import 'package:dart_ui/src/rendering/text/framework_fonts_io.dart';

void main() {
  String? packageConfig;
  String? library;
  String? resolutionError;
  try {
    packageConfig = Isolate.packageConfigSync?.toString();
    library = Isolate.resolvePackageUriSync(
        Uri.parse('package:dart_ui/dart_ui.dart'))?.toString();
  } catch (error) {
    resolutionError = error.toString();
  }
  final result = FrameworkFonts.install();
  print(jsonEncode({
    'cwd': Directory.current.path,
    'packageConfig': packageConfig,
    'library': library,
    'resolutionError': resolutionError,
    'fontDirectory': result.directory,
    'uiFontLoaded': result.uiFontLoaded,
    'iconFontLoaded': result.iconFontLoaded,
    'phosphorIconFontLoaded': result.phosphorIconFontLoaded,
    'fluentIconFontLoaded': result.fluentIconFontLoaded,
  }));
}
