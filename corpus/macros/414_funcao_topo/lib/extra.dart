import 'package:macros/macros.dart';

macro class Extra implements FunctionDeclarationsMacro {
  const Extra();

  @override
  Future<void> buildDeclarationsForFunction(
      FunctionDeclaration function, DeclarationBuilder builder) async {
    builder.declareInLibrary(DeclarationCode.fromString(
        'int triplo(int valor) => valor * 3;'));
  }
}
