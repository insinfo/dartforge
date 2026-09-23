// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c18_evento_no_projetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c18_evento_no_projetado.dart' as import1;
import 'a11_projecao.template.dart' as import2;
import 'a11_projecao.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$C18EventoNoProjetado = const [];

class ViewC18EventoNoProjetado0 extends import0.ComponentView<import1.C18EventoNoProjetado> {
  late final import2.ViewA11Projecao0 _compView_0;
  late final import3.A11Projecao _A11Projecao_0_5;
  static import4.ComponentStyles? _componentStyles;
  ViewC18EventoNoProjetado0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('c18-evento-no-projetado'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/c18_evento_no_projetado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA11Projecao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A11Projecao_0_5 = import3.A11Projecao();
    final doc = import8.document;
    final _el_1 = import7.unsafeCast(doc.createElement('button'));
    final _text_2 = import9.appendText(_el_1, 'a');
    final _el_3 = import7.unsafeCast(doc.createElement('span'));
    final _text_4 = import9.appendText(_el_3, 'b');
    this._compView_0.createAndProject(this._A11Projecao_0_5, [
      <Object>[_el_1, _el_3]
    ]);
    _el_1.addEventListener('click', this.eventHandler0(_ctx.clicou));
    _el_3.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final _ctx = this.ctx;
    _ctx.n = 1;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$C18EventoNoProjetado, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C18EventoNoProjetadoNgFactory = ComponentFactory<import1.C18EventoNoProjetado>('c18-evento-no-projetado', viewFactory_C18EventoNoProjetadoHost0);
ComponentFactory<import1.C18EventoNoProjetado> get C18EventoNoProjetadoNgFactory {
  return _C18EventoNoProjetadoNgFactory;
}

ComponentFactory<import1.C18EventoNoProjetado> createC18EventoNoProjetadoFactory() {
  return ComponentFactory('c18-evento-no-projetado', viewFactory_C18EventoNoProjetadoHost0);
}

final List<Object> styles$C18EventoNoProjetadoHost = const [];

class _ViewC18EventoNoProjetadoHost0 extends import11.HostView<import1.C18EventoNoProjetado> {
  @override
  void build() {
    this.componentView = ViewC18EventoNoProjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C18EventoNoProjetado();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.C18EventoNoProjetado> viewFactory_C18EventoNoProjetadoHost0() {
  return _ViewC18EventoNoProjetadoHost0();
}
