import 'package:macros/macros.dart';

macro class Gera implements LibraryDeclarationsMacro {
  const Gera();

  @override
  Future<void> buildDeclarationsForLibrary(
      Library library, DeclarationBuilder builder) async {
    builder.declareInLibrary(DeclarationCode.fromString(
        'const int valor = 11;'));
  }
}
