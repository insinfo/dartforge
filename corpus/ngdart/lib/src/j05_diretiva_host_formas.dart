import 'package:ngdart/angular.dart';

/// Sonda: `@HostBinding` de diretiva com `attr.x`, propriedade e sem
/// argumento.
@Directive(selector: '[j05-host]')
class J05DiretivaHostFormas {
  @HostBinding('attr.role')
  String papel = 'button';
  @HostBinding('title')
  String get titulo => 't';
  @HostBinding()
  String id = 'x';
  @HostBinding('class.ativo')
  bool ativo = true;
}
