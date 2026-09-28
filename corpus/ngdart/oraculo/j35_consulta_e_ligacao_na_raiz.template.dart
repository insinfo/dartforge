// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j35_consulta_e_ligacao_na_raiz.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j35_consulta_e_ligacao_na_raiz.dart' as import1;
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

final List<Object> styles$J35ConsultaELigacaoNaRaiz = const [];

class ViewJ35ConsultaELigacaoNaRaiz0 extends import0.ComponentView<import1.J35ConsultaELigacaoNaRaiz> {
  bool _viewQuery_s_0_isDirty = true;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  Object? _expr_0;
  Object? _expr_1;
  late final import4.HtmlElement _el_2;
  late final import4.HtmlElement _el_0;
  late final import4.HtmlElement _el_5;
  static import5.ComponentStyles? _componentStyles;
  ViewJ35ConsultaELigacaoNaRaiz0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j35-consulta-e-ligacao-na-raiz'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j35_consulta_e_ligacao_na_raiz.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import4.document;
    this._el_0 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'b');
    final _text_1 = import9.appendText(this._el_0, '1');
    this._el_2 = import9.appendSpan(doc, parentRenderNode);
    final _text_3 = import9.appendText(this._el_2, '0');
    final _anchor_4 = import9.appendAnchor(parentRenderNode);
    this._appEl_4 = ViewContainer(4, null, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J35ConsultaELigacaoNaRaiz1);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    this._el_5 = import9.appendElement<import4.HtmlElement>(doc, parentRenderNode, 'u');
    final _text_6 = import9.appendText(this._el_5, '3');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.a);
    }
    this._NgIf_4_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j35_consulta_e_ligacao_na_raiz.html:44:53 */;
    this._appEl_4.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_s_0_isDirty) {
        _ctx.todos = [
          this._el_2,
          ...this._appEl_4.mapNestedViewsWithSingleResult((_ViewJ35ConsultaELigacaoNaRaiz1 nestedView) {
            return nestedView._el_1;
          })
        ];
        this._viewQuery_s_0_isDirty = false;
      }
    }
    final currVal_0 = _ctx.a;
    if (import12.checkBinding(this._expr_0, currVal_0, 'a', 'package:corpus_ngdart/src/j35_consulta_e_ligacao_na_raiz.html')) {
      import9.updateClassBinding(this._el_0, 'x', currVal_0) /* REF:package:corpus_ngdart/src/j35_consulta_e_ligacao_na_raiz.html:3:16 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.a;
    if (import12.checkBinding(this._expr_1, currVal_1, 'a', 'package:corpus_ngdart/src/j35_consulta_e_ligacao_na_raiz.html')) {
      import9.updateClassBinding(this._el_5, 'y', currVal_1) /* REF:package:corpus_ngdart/src/j35_consulta_e_ligacao_na_raiz.html:74:87 */;
      this._expr_1 = currVal_1;
    }
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J35ConsultaELigacaoNaRaiz, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J35ConsultaELigacaoNaRaizNgFactory = ComponentFactory<import1.J35ConsultaELigacaoNaRaiz>('j35-consulta-e-ligacao-na-raiz', viewFactory_J35ConsultaELigacaoNaRaizHost0);
ComponentFactory<import1.J35ConsultaELigacaoNaRaiz> get J35ConsultaELigacaoNaRaizNgFactory {
  return _J35ConsultaELigacaoNaRaizNgFactory;
}

ComponentFactory<import1.J35ConsultaELigacaoNaRaiz> createJ35ConsultaELigacaoNaRaizFactory() {
  return ComponentFactory('j35-consulta-e-ligacao-na-raiz', viewFactory_J35ConsultaELigacaoNaRaizHost0);
}

class _ViewJ35ConsultaELigacaoNaRaiz1 extends import14.EmbeddedView<import1.J35ConsultaELigacaoNaRaiz> {
  late final import4.HtmlElement _el_1;
  _ViewJ35ConsultaELigacaoNaRaiz1(import15.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this._el_1 = import9.appendElement<import4.HtmlElement>(doc, _el_0, 'i');
    final _text_2 = import9.appendText(this._el_1, '2');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ35ConsultaELigacaoNaRaiz0>((this.parentView!))._viewQuery_s_0_isDirty = true;
  }
}

import14.EmbeddedView<void> viewFactory_J35ConsultaELigacaoNaRaiz1(import15.RenderView parentView, int parentIndex) {
  return _ViewJ35ConsultaELigacaoNaRaiz1(parentView, parentIndex);
}

final List<Object> styles$J35ConsultaELigacaoNaRaizHost = const [];

class _ViewJ35ConsultaELigacaoNaRaizHost0 extends import16.HostView<import1.J35ConsultaELigacaoNaRaiz> {
  @override
  void build() {
    this.componentView = ViewJ35ConsultaELigacaoNaRaiz0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J35ConsultaELigacaoNaRaiz();
    this.initRootNode(_el_0);
  }
}

import16.HostView<import1.J35ConsultaELigacaoNaRaiz> viewFactory_J35ConsultaELigacaoNaRaizHost0() {
  return _ViewJ35ConsultaELigacaoNaRaizHost0();
}
