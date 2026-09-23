// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'c10_entrada_antes_da_propriedade.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'c10_entrada_antes_da_propriedade.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'dart:html' as import4;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import16;

final List<Object> styles$C10EntradaAntesDaPropriedade = const [];

class ViewC10EntradaAntesDaPropriedade0 extends import0.ComponentView<import1.C10EntradaAntesDaPropriedade> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  Object? _expr_0;
  late final import4.DivElement _el_0;
  static import5.ComponentStyles? _componentStyles;
  ViewC10EntradaAntesDaPropriedade0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('c10-entrada-antes-da-propriedade'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/c10_entrada_antes_da_propriedade.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    this._el_0 = import9.appendDiv(doc, parentRenderNode);
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_C10EntradaAntesDaPropriedade1);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/c10_entrada_antes_da_propriedade.html:33:48 */;
    this._appEl_1.detectChangesInNestedViews();
    final currVal_0 = _ctx.titulo;
    if (import12.checkBinding(this._expr_0, currVal_0, 'titulo', 'package:corpus_ngdart/src/c10_entrada_antes_da_propriedade.html')) {
      import9.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/c10_entrada_antes_da_propriedade.html:5:21 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$C10EntradaAntesDaPropriedade, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _C10EntradaAntesDaPropriedadeNgFactory = ComponentFactory<import1.C10EntradaAntesDaPropriedade>('c10-entrada-antes-da-propriedade', viewFactory_C10EntradaAntesDaPropriedadeHost0);
ComponentFactory<import1.C10EntradaAntesDaPropriedade> get C10EntradaAntesDaPropriedadeNgFactory {
  return _C10EntradaAntesDaPropriedadeNgFactory;
}

ComponentFactory<import1.C10EntradaAntesDaPropriedade> createC10EntradaAntesDaPropriedadeFactory() {
  return ComponentFactory('c10-entrada-antes-da-propriedade', viewFactory_C10EntradaAntesDaPropriedadeHost0);
}

class _ViewC10EntradaAntesDaPropriedade1 extends import14.EmbeddedView<import1.C10EntradaAntesDaPropriedade> {
  _ViewC10EntradaAntesDaPropriedade1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    final _text_1 = import9.appendText(_el_0, 'oi');
    this.initRootNode(_el_0);
  }
}

import14.EmbeddedView<void> viewFactory_C10EntradaAntesDaPropriedade1(import15.RenderView parentView, int parentIndex) {
  return _ViewC10EntradaAntesDaPropriedade1(parentView, parentIndex);
}

final List<Object> styles$C10EntradaAntesDaPropriedadeHost = const [];

class _ViewC10EntradaAntesDaPropriedadeHost0 extends import16.HostView<import1.C10EntradaAntesDaPropriedade> {
  @override
  void build() {
    this.componentView = ViewC10EntradaAntesDaPropriedade0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.C10EntradaAntesDaPropriedade();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.C10EntradaAntesDaPropriedade> viewFactory_C10EntradaAntesDaPropriedadeHost0() {
  return _ViewC10EntradaAntesDaPropriedadeHost0();
}
