import 'package:ngdart/angular.dart';

/// `@Directive` com `@HostBinding`: gera um `DirectiveChangeDetector`.
@Directive(selector: '[e02-host]')
class E02DiretivaComHostBinding {
  @HostBinding('class.ativo')
  bool ativo = true;
}
