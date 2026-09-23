import 'package:ngdart/angular.dart';

/// `@Directive` só com `@HostListener`: sem `@HostBinding` não há
/// `DirectiveChangeDetector`, e o arquivo gerado é o trivial.
@Directive(selector: '[e05-listener]')
class E05DiretivaSoListener {
  @HostListener('click')
  void clicou() {}
}