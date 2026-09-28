import 'package:ngdart/angular.dart';

class J40Rastro {}

/// Diretiva com `@HostBinding` que pede um serviço de fora da visão: a
/// criação embrulhada no `XNgCd`.
@Directive(selector: '[j40-com-host]')
class J40ComHost {
  final J40Rastro rastro;

  J40ComHost(this.rastro);

  @HostBinding('class.ativo')
  bool ativo = true;
}
