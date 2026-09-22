import 'dart:html';

import 'package:ngdart/angular.dart';

/// `@Directive` sem nada de hospedeiro: o oficial não gera visão nenhuma.
@Directive(selector: '[e01-simples]')
class E01DiretivaSimples {
  final Element el;

  E01DiretivaSimples(this.el);
}
