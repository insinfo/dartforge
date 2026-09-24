import 'package:macros/macros.dart';

macro class Gera implements ClassTypesMacro {
  const Gera();

  @override
  Future<void> buildTypesForClass(
      ClassDeclaration clazz, ClassTypeBuilder builder) async {
    builder.declareType('Extra', DeclarationCode.fromString(
        'class Extra { static const int valor = 7; }'));
  }
}
