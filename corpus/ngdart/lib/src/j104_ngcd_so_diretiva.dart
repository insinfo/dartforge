import 'package:ngdart/angular.dart';

/// Arquivo só de diretiva: com a imutável antes da dinâmica, o
/// `dom_helpers` é alocado antes do `check_binding` (ordem do texto).
@Directive(selector: '[j104Lista]')
class J104Lista {
  @HostBinding('attr.role')
  final String role = 'list';

  @HostBinding('attr.ignoreUpAndDown')
  bool ignorar = false;
}
