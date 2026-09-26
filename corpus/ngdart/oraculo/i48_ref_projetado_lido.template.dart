// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i48_ref_projetado_lido.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i48_ref_projetado_lido.dart' as import1;
import 'a11_projecao.template.dart' as import2;
import 'a11_projecao.dart' as import3;
import 'a02_texto_estatico.template.dart' as import4;
import 'a02_texto_estatico.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import7;
import 'package:ngdart/src/core/linker/views/view.dart' as import8;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import9;
import 'package:ngdart/src/utilities.dart' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$I48RefProjetadoLido = const [];

class ViewI48RefProjetadoLido0 extends import0.ComponentView<import1.I48RefProjetadoLido> {
  late final import2.ViewA11Projecao0 _compView_0;
  late final import3.A11Projecao _A11Projecao_0_5;
  late final import4.ViewA02TextoEstatico0 _compView_2;
  late final import5.A02TextoEstatico _A02TextoEstatico_2_5;
  late final import6.InputElement _el_1;
  static import7.ComponentStyles? _componentStyles;
  ViewI48RefProjetadoLido0(import8.View parentView, int parentIndex) : super(parentView, parentIndex, import9.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import10.unsafeCast(import6.document.createElement('i48-ref-projetado-lido'));
  }
  static String? get _debugComponentUrl {
    return (import10.isDevMode ? 'asset:corpus_ngdart/lib/src/i48_ref_projetado_lido.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA11Projecao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A11Projecao_0_5 = import3.A11Projecao();
    final doc = import6.document;
    this._el_1 = import10.unsafeCast(doc.createElement('input'));
    this._compView_2 = import4.ViewA02TextoEstatico0(this, 2);
    final _el_2 = this._compView_2.rootElement;
    this._A02TextoEstatico_2_5 = import5.A02TextoEstatico();
    this._compView_2.create(this._A02TextoEstatico_2_5);
    this._compView_0.createAndProject(this._A11Projecao_0_5, [
      <Object>[this._el_1, _el_2]
    ]);
    final _el_3 = import11.appendElement<import6.ButtonElement>(doc, parentRenderNode, 'button');
    final _text_4 = import11.appendText(_el_3, 'x');
    _el_3.addEventListener('click', this.eventHandler1(this._handleEvent_0));
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_2.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_2.destroyInternalState();
  }

  void _handleEvent_0($event) {
    final local_campo = this._el_1;
    final local_filho = this._A02TextoEstatico_2_5;
    final _ctx = this.ctx;
    _ctx.usar(local_campo.value, local_filho);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import7.ComponentStyles.unscoped(styles$I48RefProjetadoLido, _debugComponentUrl));
      if (import10.isDevMode) {
        import7.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I48RefProjetadoLidoNgFactory = ComponentFactory<import1.I48RefProjetadoLido>('i48-ref-projetado-lido', viewFactory_I48RefProjetadoLidoHost0);
ComponentFactory<import1.I48RefProjetadoLido> get I48RefProjetadoLidoNgFactory {
  return _I48RefProjetadoLidoNgFactory;
}

ComponentFactory<import1.I48RefProjetadoLido> createI48RefProjetadoLidoFactory() {
  return ComponentFactory('i48-ref-projetado-lido', viewFactory_I48RefProjetadoLidoHost0);
}

final List<Object> styles$I48RefProjetadoLidoHost = const [];

class _ViewI48RefProjetadoLidoHost0 extends import13.HostView<import1.I48RefProjetadoLido> {
  @override
  void build() {
    this.componentView = ViewI48RefProjetadoLido0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I48RefProjetadoLido();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.I48RefProjetadoLido> viewFactory_I48RefProjetadoLidoHost0() {
  return _ViewI48RefProjetadoLidoHost0();
}
