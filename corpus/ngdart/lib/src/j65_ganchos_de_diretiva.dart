import 'package:ngdart/angular.dart';

/// Diretiva com os ganchos de conteúdo e de visão (o
/// `LiDropdownMenuPositionDirective` do limitless_ui): chamados depois dos
/// filhos do nó, de baixo para cima.
@Directive(selector: '[j65-todos]')
class J65Todos
    implements
        AfterContentInit,
        AfterContentChecked,
        AfterViewInit,
        AfterViewChecked,
        OnDestroy {
  @override
  void ngAfterContentInit() {}
  @override
  void ngAfterContentChecked() {}
  @override
  void ngAfterViewInit() {}
  @override
  void ngAfterViewChecked() {}
  @override
  void ngOnDestroy() {}
}

@Directive(selector: '[j65-conteudo]')
class J65Conteudo implements AfterContentInit, OnDestroy {
  @override
  void ngAfterContentInit() {}
  @override
  void ngOnDestroy() {}
}

/// Com `@HostBinding`: o gancho lê a instância do `XNgCd`.
@Directive(selector: '[j65-hospedeiro]')
class J65Hospedeiro implements AfterViewInit {
  @HostBinding('class.ativo')
  bool ativo = true;

  @override
  void ngAfterViewInit() {}
}

@Component(
  selector: 'j65-filho',
  template: '<ng-content></ng-content>',
)
class J65Filho implements AfterContentInit, OnDestroy {
  @override
  void ngAfterContentInit() {}
  @override
  void ngOnDestroy() {}
}

@Component(
  selector: 'j65-ganchos-de-diretiva',
  templateUrl: 'j65_ganchos_de_diretiva.html',
  directives: [J65Filho, J65Todos, J65Conteudo, J65Hospedeiro, NgIf],
)
class J65GanchosDeDiretiva {
  bool mostrar = true;
}
