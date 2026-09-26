// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i46_view_child_projetado.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i46_view_child_projetado.dart' as import1;
import 'a11_projecao.template.dart' as import2;
import 'a11_projecao.dart' as import3;
import 'a02_texto_estatico.template.dart' as import4;
import 'a02_texto_estatico.dart' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import13;

final List<Object> styles$I46ViewChildProjetado = const [];

class ViewI46ViewChildProjetado0 extends import0.ComponentView<import1.I46ViewChildProjetado> {
  late final import2.ViewA11Projecao0 _compView_0;
  late final import3.A11Projecao _A11Projecao_0_5;
  late final import4.ViewA02TextoEstatico0 _compView_3;
  late final import5.A02TextoEstatico _A02TextoEstatico_3_5;
  static import6.ComponentStyles? _componentStyles;
  ViewI46ViewChildProjetado0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i46-view-child-projetado'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i46_view_child_projetado.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = import2.ViewA11Projecao0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._A11Projecao_0_5 = import3.A11Projecao();
    final doc = import10.document;
    final _el_1 = import9.unsafeCast(doc.createElement('div'));
    final _text_2 = import11.appendText(_el_1, 'x');
    this._compView_3 = import4.ViewA02TextoEstatico0(this, 3);
    final _el_3 = this._compView_3.rootElement;
    this._A02TextoEstatico_3_5 = import5.A02TextoEstatico();
    this._compView_3.create(this._A02TextoEstatico_3_5);
    this._compView_0.createAndProject(this._A11Projecao_0_5, [
      <Object>[_el_1, _el_3]
    ]);
    _ctx.caixa = _el_1;
    _ctx.filho = this._A02TextoEstatico_3_5;
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_3.detectChanges();
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_3.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I46ViewChildProjetado, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I46ViewChildProjetadoNgFactory = ComponentFactory<import1.I46ViewChildProjetado>('i46-view-child-projetado', viewFactory_I46ViewChildProjetadoHost0);
ComponentFactory<import1.I46ViewChildProjetado> get I46ViewChildProjetadoNgFactory {
  return _I46ViewChildProjetadoNgFactory;
}

ComponentFactory<import1.I46ViewChildProjetado> createI46ViewChildProjetadoFactory() {
  return ComponentFactory('i46-view-child-projetado', viewFactory_I46ViewChildProjetadoHost0);
}

final List<Object> styles$I46ViewChildProjetadoHost = const [];

class _ViewI46ViewChildProjetadoHost0 extends import13.HostView<import1.I46ViewChildProjetado> {
  @override
  void build() {
    this.componentView = ViewI46ViewChildProjetado0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I46ViewChildProjetado();
    this.initRootNode(_el_0);
  }
}

import13.HostView<import1.I46ViewChildProjetado> viewFactory_I46ViewChildProjetadoHost0() {
  return _ViewI46ViewChildProjetadoHost0();
}
