// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j72_consultas_de_diretiva.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j72_consultas_de_diretiva.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/devtools.dart' as import8;
import 'package:ngdart/src/runtime/check_binding.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;
import 'package:ngdart/src/core/change_detection/directive_change_detector.dart' as import12;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import13;

final List<Object> styles$J72ConsultasDeDiretiva = const [];

class ViewJ72ConsultasDeDiretiva0 extends import0.ComponentView<import1.J72ConsultasDeDiretiva> {
  late final import1.J72Grupo _J72Grupo_0_5;
  late final J72ItemNgCd _J72Item_1_5;
  late final J72ItemNgCd _J72Item_2_5;
  late final import1.J72Marca _J72Marca_3_5;
  late final J72ItemNgCd _J72Item_5_5;
  late final import1.J72Marca _J72Marca_5_6;
  late final import1.J72Grupo _J72Grupo_6_5;
  late final import2.HtmlElement _el_1;
  late final import2.HtmlElement _el_2;
  late final import2.HtmlElement _el_5;
  static import3.ComponentStyles? _componentStyles;
  ViewJ72ConsultasDeDiretiva0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('j72-consultas-de-diretiva'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j72_consultas_de_diretiva.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    import7.setAttribute(_el_0, 'j72-grupo', '');
    this._J72Grupo_0_5 = import1.J72Grupo();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_0, this._J72Grupo_0_5);
    }
    this._el_1 = import7.appendElement<import2.HtmlElement>(doc, _el_0, 'p');
    import7.setAttribute(this._el_1, 'j72-item', '');
    this._J72Item_1_5 = J72ItemNgCd(import1.J72Item());
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_1, this._J72Item_1_5.instance);
    }
    this._el_2 = import7.appendSpan(doc, this._el_1);
    import7.setAttribute(this._el_2, 'j72-item', '');
    this._J72Item_2_5 = J72ItemNgCd(import1.J72Item());
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_2, this._J72Item_2_5.instance);
    }
    final _el_3 = import7.appendElement<import2.HtmlElement>(doc, _el_0, 'b');
    import7.setAttribute(_el_3, 'j72-marca', '');
    this._J72Marca_3_5 = import1.J72Marca();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_3, this._J72Marca_3_5);
    }
    final _el_4 = import7.appendElement<import2.HtmlElement>(doc, _el_0, 'section');
    this._el_5 = import7.appendElement<import2.HtmlElement>(doc, _el_4, 'i');
    import7.setAttribute(this._el_5, 'j72-item', '');
    import7.setAttribute(this._el_5, 'j72-marca', '');
    this._J72Item_5_5 = J72ItemNgCd(import1.J72Item());
    this._J72Marca_5_6 = import1.J72Marca();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(this._el_5, this._J72Item_5_5.instance);
      import8.Inspector.instance.registerDirective(this._el_5, this._J72Marca_5_6);
    }
    this._J72Grupo_0_5.pelo = [this._J72Item_1_5.instance, this._J72Item_2_5.instance, this._J72Item_5_5.instance];
    this._J72Grupo_0_5.diretos = [this._J72Item_1_5.instance, this._J72Item_5_5.instance];
    this._J72Grupo_0_5.todos = [this._J72Item_1_5.instance, this._J72Item_2_5.instance, this._J72Item_5_5.instance];
    this._J72Grupo_0_5.marca = this._J72Marca_3_5;
    this._J72Grupo_0_5.nos = [_el_3, this._el_5];
    this._J72Grupo_0_5.nada = [];
    final _el_6 = import7.appendElement<import2.UListElement>(doc, parentRenderNode, 'ul');
    import7.setAttribute(_el_6, 'j72-grupo', '');
    this._J72Grupo_6_5 = import1.J72Grupo();
    if (import8.isDevToolsEnabled) {
      import8.Inspector.instance.registerDirective(_el_6, this._J72Grupo_6_5);
    }
    this._J72Grupo_6_5.pelo = [];
    this._J72Grupo_6_5.diretos = [];
    this._J72Grupo_6_5.todos = [];
    this._J72Grupo_6_5.nos = [];
    this._J72Grupo_6_5.nada = [];
  }

  @override
  void detectChangesInternal() {
    bool firstCheck = this.firstCheck;
    if ((!import9.debugThrowIfChanged)) {
      if (firstCheck) {
        this._J72Grupo_0_5.ngAfterContentInit();
        this._J72Grupo_6_5.ngAfterContentInit();
      }
    }
    this._J72Item_1_5.detectHostChanges(this, this._el_1);
    this._J72Item_2_5.detectHostChanges(this, this._el_2);
    this._J72Item_5_5.detectHostChanges(this, this._el_5);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J72ConsultasDeDiretiva, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J72ConsultasDeDiretivaNgFactory = ComponentFactory<import1.J72ConsultasDeDiretiva>('j72-consultas-de-diretiva', viewFactory_J72ConsultasDeDiretivaHost0);
ComponentFactory<import1.J72ConsultasDeDiretiva> get J72ConsultasDeDiretivaNgFactory {
  return _J72ConsultasDeDiretivaNgFactory;
}

ComponentFactory<import1.J72ConsultasDeDiretiva> createJ72ConsultasDeDiretivaFactory() {
  return ComponentFactory('j72-consultas-de-diretiva', viewFactory_J72ConsultasDeDiretivaHost0);
}

final List<Object> styles$J72ConsultasDeDiretivaHost = const [];

class _ViewJ72ConsultasDeDiretivaHost0 extends import11.HostView<import1.J72ConsultasDeDiretiva> {
  @override
  void build() {
    this.componentView = ViewJ72ConsultasDeDiretiva0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J72ConsultasDeDiretiva();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J72ConsultasDeDiretiva> viewFactory_J72ConsultasDeDiretivaHost0() {
  return _ViewJ72ConsultasDeDiretivaHost0();
}

class J72ItemNgCd extends import12.DirectiveChangeDetector {
  final import1.J72Item instance;
  Object? _expr_0;
  J72ItemNgCd(this.instance);
  void detectHostChanges(import13.RenderView view, import2.Element el) {
    final currVal_0 = this.instance.item;
    if (import9.checkBinding(this._expr_0, currVal_0, null, null)) {
      import7.updateClassBindingNonHtml(el, 'item', currVal_0);
      this._expr_0 = currVal_0;
    }
  }
}
