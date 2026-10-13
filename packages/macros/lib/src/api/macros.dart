part of '../api.dart';

// The macro interfaces (spec, "Macro phases" and "Ordering"): one per pair
// (target, phase). A `macro class` implements the ones it needs; the host
// calls the current phase's method for each application whose target matches.
//
// Phases: *types* (names only; declares new types and adds supertypes),
// *declarations* (member introspection; declares members and top-level
// declarations) and *definitions* (completes bodies with `augment`).

/// Marker interface of every macro.
abstract interface class Macro {}

// -- Library (applied to the `library` directive) -----------------------------

abstract interface class LibraryTypesMacro implements Macro {
  FutureOr<void> buildTypesForLibrary(Library library, TypeBuilder builder);
}

abstract interface class LibraryDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForLibrary(
      Library library, DeclarationBuilder builder);
}

abstract interface class LibraryDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForLibrary(
      Library library, LibraryDefinitionBuilder builder);
}

// -- Top-level functions, instance and static methods ------------------------

abstract interface class FunctionTypesMacro implements Macro {
  FutureOr<void> buildTypesForFunction(
      FunctionDeclaration function, TypeBuilder builder);
}

abstract interface class FunctionDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForFunction(
      FunctionDeclaration function, DeclarationBuilder builder);
}

abstract interface class FunctionDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForFunction(
      FunctionDeclaration function, FunctionDefinitionBuilder builder);
}

// -- Top-level variables and fields -------------------------------------------

abstract interface class VariableTypesMacro implements Macro {
  FutureOr<void> buildTypesForVariable(
      VariableDeclaration variable, TypeBuilder builder);
}

abstract interface class VariableDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForVariable(
      VariableDeclaration variable, DeclarationBuilder builder);
}

abstract interface class VariableDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForVariable(
      VariableDeclaration variable, VariableDefinitionBuilder builder);
}

// -- Classes ------------------------------------------------------------------

abstract interface class ClassTypesMacro implements Macro {
  FutureOr<void> buildTypesForClass(
      ClassDeclaration clazz, ClassTypeBuilder builder);
}

abstract interface class ClassDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForClass(
      ClassDeclaration clazz, MemberDeclarationBuilder builder);
}

abstract interface class ClassDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForClass(
      ClassDeclaration clazz, TypeDefinitionBuilder builder);
}

// -- Enums and enum values -----------------------------------------------------

abstract interface class EnumTypesMacro implements Macro {
  FutureOr<void> buildTypesForEnum(
      EnumDeclaration enuum, EnumTypeBuilder builder);
}

abstract interface class EnumDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForEnum(
      EnumDeclaration enuum, EnumDeclarationBuilder builder);
}

abstract interface class EnumDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForEnum(
      EnumDeclaration enuum, EnumDefinitionBuilder builder);
}

abstract interface class EnumValueTypesMacro implements Macro {
  FutureOr<void> buildTypesForEnumValue(
      EnumValueDeclaration entry, TypeBuilder builder);
}

abstract interface class EnumValueDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForEnumValue(
      EnumValueDeclaration entry, EnumDeclarationBuilder builder);
}

abstract interface class EnumValueDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForEnumValue(
      EnumValueDeclaration entry, EnumValueDefinitionBuilder builder);
}

// -- Fields, methods and constructors (members) -------------------------------

abstract interface class FieldTypesMacro implements Macro {
  FutureOr<void> buildTypesForField(
      FieldDeclaration field, TypeBuilder builder);
}

abstract interface class FieldDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForField(
      FieldDeclaration field, MemberDeclarationBuilder builder);
}

abstract interface class FieldDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForField(
      FieldDeclaration field, VariableDefinitionBuilder builder);
}

abstract interface class MethodTypesMacro implements Macro {
  FutureOr<void> buildTypesForMethod(
      MethodDeclaration method, TypeBuilder builder);
}

abstract interface class MethodDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForMethod(
      MethodDeclaration method, MemberDeclarationBuilder builder);
}

abstract interface class MethodDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForMethod(
      MethodDeclaration method, FunctionDefinitionBuilder builder);
}

abstract interface class ConstructorTypesMacro implements Macro {
  FutureOr<void> buildTypesForConstructor(
      ConstructorDeclaration constructor, TypeBuilder builder);
}

abstract interface class ConstructorDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForConstructor(
      ConstructorDeclaration constructor, MemberDeclarationBuilder builder);
}

abstract interface class ConstructorDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForConstructor(
      ConstructorDeclaration constructor, ConstructorDefinitionBuilder builder);
}

// -- Mixins ---------------------------------------------------------------------

abstract interface class MixinTypesMacro implements Macro {
  FutureOr<void> buildTypesForMixin(
      MixinDeclaration mixin, MixinTypeBuilder builder);
}

abstract interface class MixinDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForMixin(
      MixinDeclaration mixin, MemberDeclarationBuilder builder);
}

abstract interface class MixinDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForMixin(
      MixinDeclaration mixin, TypeDefinitionBuilder builder);
}

// -- Extensions and extension types ------------------------------------------------

abstract interface class ExtensionTypesMacro implements Macro {
  FutureOr<void> buildTypesForExtension(
      ExtensionDeclaration extension, TypeBuilder builder);
}

abstract interface class ExtensionDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForExtension(
      ExtensionDeclaration extension, MemberDeclarationBuilder builder);
}

abstract interface class ExtensionDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForExtension(
      ExtensionDeclaration extension, TypeDefinitionBuilder builder);
}

abstract interface class ExtensionTypeTypesMacro implements Macro {
  FutureOr<void> buildTypesForExtensionType(
      ExtensionTypeDeclaration extension, TypeBuilder builder);
}

abstract interface class ExtensionTypeDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForExtensionType(
      ExtensionTypeDeclaration extension, MemberDeclarationBuilder builder);
}

abstract interface class ExtensionTypeDefinitionMacro implements Macro {
  FutureOr<void> buildDefinitionForExtensionType(
      ExtensionTypeDeclaration extension, TypeDefinitionBuilder builder);
}

// -- Typedefs -------------------------------------------------------------------

abstract interface class TypeAliasTypesMacro implements Macro {
  FutureOr<void> buildTypesForTypeAlias(
      TypeAliasDeclaration declaration, TypeBuilder builder);
}

abstract interface class TypeAliasDeclarationsMacro implements Macro {
  FutureOr<void> buildDeclarationsForTypeAlias(
      TypeAliasDeclaration declaration, DeclarationBuilder builder);
}
