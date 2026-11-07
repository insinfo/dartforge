import 'dart:core' as core;
class Deprecated { const Deprecated(core.String text); }
const deprecated = 0;
const legacy = core.Deprecated('old');
typedef AliasDeprecated = core.Deprecated;
@core.Deprecated('old') class Old {}
@core.Deprecated.new('old') class ExplicitNew {}
@core.Deprecated.extend('migration') class ExtendingOnly {}
@core.Deprecated.notThere('missing') class InvalidConstructor {}
@core.deprecated class CoreConstant {}
@core.deprecated() class InvalidConstantCall {}
@Deprecated('custom') class CustomAnnotation {}
@deprecated class CustomConstant {}
@legacy class ConstantAlias {}
@AliasDeprecated('alias constructor') class ConstructorAlias {}
@core.Deprecated class MissingCall {}
@core.Deprecated() class MissingMessage {}
@AliasDeprecated.new('alias new') class AliasNew {}
@AliasDeprecated.notThere('alias missing') class AliasMissing {}
@core.Deprecated.optional('optional') class OptionalOnly {}
class Generic<T extends core.Object?> {
  Generic();
  Generic.named(core.int value, {required core.String title});
  core.String method<U>(core.int first, [core.String second = 'x']) => '';
  core.int get getter => 1;
  set setter(core.int value) {}
}
void function<T>(core.List<T> values, {core.String text = 'a', required core.int count}) {}
void trivia(
  @core.deprecated core.int /* inside */ first, {
  core.String text = "a(b)",
  core.List<core.int> list = const [1, 2],
}) {}
void callback(void Function(core.int, [core.String]) action, [core.String text = 'x)y']) {}
void untyped(first, {second = 2}) {}
void functionDefaults({core.Object callback = identity}) {}
core.int identity(core.int x) => x;


void use(core.List< > values) {}
