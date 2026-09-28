import 'package:ngdart/angular.dart';

import 'j26_gatilho.dart';

/// Componente que recebe o `<template j26Gatilho>` pelo conteúdo
/// (`@ContentChild`) e o desenha com `ngTemplateOutlet` num `<template>`
/// escrito com duas ligações.
@Component(
  selector: 'j27-caixa',
  templateUrl: 'j27_caixa.html',
  directives: [NgTemplateOutlet],
)
class J27Caixa {
  @ContentChild(J26Gatilho)
  J26Gatilho? gatilho;

  Object? contexto = 'c';
}
