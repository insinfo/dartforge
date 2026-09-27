// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j18_consulta_e_ligacao_em_if.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j18_consulta_e_ligacao_em_if.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/src/runtime/queries.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J18ConsultaELigacaoEmIf = const [];

class ViewJ18ConsultaELigacaoEmIf0 extends import0.ComponentView<import1.J18ConsultaELigacaoEmIf> {
  bool _viewQuery_area_0_isDirty = true;
  bool _viewQuery_matiz_1_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  static import4.ComponentStyles? _componentStyles;
  ViewJ18ConsultaELigacaoEmIf0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('j18-consulta-e-ligacao-em-if'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/j18_consulta_e_ligacao_em_if.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J18ConsultaELigacaoEmIf1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.a);
    }
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html:5:14 */;
    this._appEl_0.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_area_0_isDirty) {
        _ctx.area = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ18ConsultaELigacaoEmIf1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_area_0_isDirty = false;
      }
      if (this._viewQuery_matiz_1_isDirty) {
        _ctx.matiz = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ18ConsultaELigacaoEmIf1 nestedView) {
          return nestedView._el_3;
        }));
        this._viewQuery_matiz_1_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$J18ConsultaELigacaoEmIf, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J18ConsultaELigacaoEmIfNgFactory = ComponentFactory<import1.J18ConsultaELigacaoEmIf>('j18-consulta-e-ligacao-em-if', viewFactory_J18ConsultaELigacaoEmIfHost0);
ComponentFactory<import1.J18ConsultaELigacaoEmIf> get J18ConsultaELigacaoEmIfNgFactory {
  return _J18ConsultaELigacaoEmIfNgFactory;
}

ComponentFactory<import1.J18ConsultaELigacaoEmIf> createJ18ConsultaELigacaoEmIfFactory() {
  return ComponentFactory('j18-consulta-e-ligacao-em-if', viewFactory_J18ConsultaELigacaoEmIfHost0);
}

class _ViewJ18ConsultaELigacaoEmIf1 extends import15.EmbeddedView<import1.J18ConsultaELigacaoEmIf> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import8.DivElement _el_1;
  late final import8.DivElement _el_3;
  late final import8.DivElement _el_2;
  late final import8.DivElement _el_4;
  _ViewJ18ConsultaELigacaoEmIf1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _ctx = this.ctx;
    final doc = import8.document;
    final _el_0 = import7.unsafeCast(doc.createElement('div'));
    this.updateChildClass(_el_0, 'topo');
    this._el_1 = import9.appendDiv(doc, _el_0);
    this.updateChildClass(this._el_1, 'cor');
    this._el_2 = import9.appendDiv(doc, this._el_1);
    this.updateChildClass(this._el_2, 'ponto');
    this._el_3 = import9.appendDiv(doc, _el_0);
    this.updateChildClass(this._el_3, 'matiz');
    this._el_4 = import9.appendDiv(doc, this._el_3);
    this.updateChildClass(this._el_4, 'barra');
    this._el_1.addEventListener('mousedown', this.eventHandler1(_ctx.descer));
    this._el_3.addEventListener('mousedown', this.eventHandler1(_ctx.descer));
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.marcado;
    if (import12.checkBinding(this._expr_0, currVal_0, 'marcado', 'package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html')) {
      import9.updateClassBinding(this._el_1, 'ativa', currVal_0) /* REF:package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html:56:79 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.marcado;
    if (import12.checkBinding(this._expr_1, currVal_1, 'marcado', 'package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html')) {
      import9.updateClassBinding(this._el_2, 'oculto', currVal_1) /* REF:package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html:137:161 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = (!_ctx.marcado);
    if (import12.checkBinding(this._expr_2, currVal_2, '!marcado', 'package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html')) {
      import9.updateClassBinding(this._el_4, 'oculto', currVal_2) /* REF:package:corpus_ngdart/src/j18_consulta_e_ligacao_em_if.html:267:292 */;
      this._expr_2 = currVal_2;
    }
  }

  @override
  void dirtyParentQueriesInternal() {
    import7.unsafeCast<ViewJ18ConsultaELigacaoEmIf0>((this.parentView!))._viewQuery_area_0_isDirty = true;
    import7.unsafeCast<ViewJ18ConsultaELigacaoEmIf0>((this.parentView!))._viewQuery_matiz_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J18ConsultaELigacaoEmIf1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ18ConsultaELigacaoEmIf1(parentView, parentIndex);
}

final List<Object> styles$J18ConsultaELigacaoEmIfHost = const [];

class _ViewJ18ConsultaELigacaoEmIfHost0 extends import17.HostView<import1.J18ConsultaELigacaoEmIf> {
  @override
  void build() {
    this.componentView = ViewJ18ConsultaELigacaoEmIf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J18ConsultaELigacaoEmIf();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J18ConsultaELigacaoEmIf> viewFactory_J18ConsultaELigacaoEmIfHost0() {
  return _ViewJ18ConsultaELigacaoEmIfHost0();
}
