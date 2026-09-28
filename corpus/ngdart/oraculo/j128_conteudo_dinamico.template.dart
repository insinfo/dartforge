// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j128_conteudo_dinamico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j128_conteudo_dinamico.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import8;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/queries.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/common/directives/ng_for.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;

final List<Object> styles$J128Caixa = const [];

class ViewJ128Caixa0 extends import0.ComponentView<import1.J128Caixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ128Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j128-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this.project(parentRenderNode, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J128Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J128CaixaNgFactory = ComponentFactory<import1.J128Caixa>('j128-caixa', viewFactory_J128CaixaHost0);
ComponentFactory<import1.J128Caixa> get J128CaixaNgFactory {
  return _J128CaixaNgFactory;
}

ComponentFactory<import1.J128Caixa> createJ128CaixaFactory() {
  return ComponentFactory('j128-caixa', viewFactory_J128CaixaHost0);
}

final List<Object> styles$J128CaixaHost = const [];

class _ViewJ128CaixaHost0 extends import8.HostView<import1.J128Caixa> {
  @override
  void build() {
    this.componentView = ViewJ128Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J128Caixa();
    this.component.itens = [];
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J128Caixa> viewFactory_J128CaixaHost0() {
  return _ViewJ128CaixaHost0();
}

final List<Object> styles$J128Usa = const [];

class ViewJ128Usa0 extends import0.ComponentView<import1.J128Usa> {
  bool _query_J128Item_0_0_isDirty = true;
  bool _query_J128Item_5_0_isDirty = true;
  bool _query_J128Item_7_0_isDirty = true;
  late dynamic _J128Servico_10_7 = import1.fabricaDeServico();
  bool _query_J128Item_10_0_isDirty = true;
  bool _viewQuery_r_0_isDirty = true;
  late final import1.J128Lista _J128Lista_0_5;
  late final import1.J128Marcador _J128Marcador_1_5;
  late final ViewContainer _appEl_4;
  late final NgIf _NgIf_4_9;
  late final import1.J128Um _J128Um_5_5;
  late final ViewContainer _appEl_6;
  late final NgIf _NgIf_6_9;
  late final ViewJ128Caixa0 _compView_7;
  late final import1.J128Caixa _J128Caixa_7_5;
  late final ViewContainer _appEl_8;
  late final NgIf _NgIf_8_9;
  late final ViewContainer _appEl_9;
  late final NgIf _NgIf_9_9;
  late final import1.J128Lista _J128Lista_10_5;
  late final import1.J128Prov _J128Prov_10_6;
  late final ViewContainer _appEl_11;
  late final NgIf _NgIf_11_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ128Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j128-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import11.appendDiv(doc, parentRenderNode);
    import11.setAttribute(_el_0, 'j128Lista', '');
    this._J128Lista_0_5 = import1.J128Lista();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J128Lista_0_5);
    }
    final _el_1 = import11.appendSpan(doc, _el_0);
    import11.setAttribute(_el_1, 'j128Marcador', '');
    this._J128Marcador_1_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_1, this._J128Marcador_1_5);
    }
    final _text_2 = import11.appendText(_el_1, 'a');
    final _text_3 = import11.appendText(_el_0, ' ');
    final _anchor_4 = import11.appendAnchor(_el_0);
    this._appEl_4 = ViewContainer(4, 0, this, _anchor_4);
    var _TemplateRef_4_8 = TemplateRef(this._appEl_4, viewFactory_J128Usa1);
    this._NgIf_4_9 = NgIf(this._appEl_4, _TemplateRef_4_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_4, this._NgIf_4_9);
    }
    final _el_5 = import11.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    import11.setAttribute(_el_5, 'j128Um', '');
    this._J128Um_5_5 = import1.J128Um();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_5, this._J128Um_5_5);
    }
    final _anchor_6 = import11.appendAnchor(_el_5);
    this._appEl_6 = ViewContainer(6, 5, this, _anchor_6);
    var _TemplateRef_6_8 = TemplateRef(this._appEl_6, viewFactory_J128Usa3);
    this._NgIf_6_9 = NgIf(this._appEl_6, _TemplateRef_6_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_6, this._NgIf_6_9);
    }
    this._compView_7 = ViewJ128Caixa0(this, 7);
    final _el_7 = this._compView_7.rootElement;
    parentRenderNode.append(_el_7);
    this._J128Caixa_7_5 = import1.J128Caixa();
    final _anchor_8 = import11.createAnchor();
    this._appEl_8 = ViewContainer(8, 7, this, _anchor_8);
    var _TemplateRef_8_8 = TemplateRef(this._appEl_8, viewFactory_J128Usa4);
    this._NgIf_8_9 = NgIf(this._appEl_8, _TemplateRef_8_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_8, this._NgIf_8_9);
    }
    this._compView_7.createAndProject(this._J128Caixa_7_5, [
      <Object>[this._appEl_8]
    ]);
    final _anchor_9 = import11.appendAnchor(parentRenderNode);
    this._appEl_9 = ViewContainer(9, null, this, _anchor_9);
    var _TemplateRef_9_8 = TemplateRef(this._appEl_9, viewFactory_J128Usa5);
    this._NgIf_9_9 = NgIf(this._appEl_9, _TemplateRef_9_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_9, this._NgIf_9_9);
    }
    final _el_10 = import11.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'nav');
    import11.setAttribute(_el_10, 'j128Lista', '');
    import11.setAttribute(_el_10, 'j128Prov', '');
    this._J128Lista_10_5 = import1.J128Lista();
    this._J128Prov_10_6 = import1.J128Prov();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_10, this._J128Lista_10_5);
      import12.Inspector.instance.registerDirective(_el_10, this._J128Prov_10_6);
    }
    final _anchor_11 = import11.appendAnchor(_el_10);
    this._appEl_11 = ViewContainer(11, 10, this, _anchor_11);
    var _TemplateRef_11_8 = TemplateRef(this._appEl_11, viewFactory_J128Usa7);
    this._NgIf_11_9 = NgIf(this._appEl_11, _TemplateRef_11_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_11, this._NgIf_11_9);
    }
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && ((1 <= nodeIndex) && (nodeIndex <= 2)))) {
      return this._J128Marcador_1_5;
    }
    if ((identical(token, import1.J128Servico) && ((10 <= nodeIndex) && (nodeIndex <= 11)))) {
      return this._J128Servico_10_7;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_4_9, 'ngIf', _ctx.x);
    }
    this._NgIf_4_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1516:1526 */;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_6_9, 'ngIf', _ctx.x);
    }
    this._NgIf_6_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1602:1611 */;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_8_9, 'ngIf', _ctx.x);
    }
    this._NgIf_8_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1650:1659 */;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_9_9, 'ngIf', _ctx.x);
    }
    this._NgIf_9_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1697:1706 */;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_11_9, 'ngIf', _ctx.x);
    }
    this._NgIf_11_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1803:1812 */;
    this._appEl_4.detectChangesInNestedViews();
    this._appEl_6.detectChangesInNestedViews();
    this._appEl_8.detectChangesInNestedViews();
    this._appEl_9.detectChangesInNestedViews();
    this._appEl_11.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._query_J128Item_0_0_isDirty) {
        this._J128Lista_0_5.itens = [
          this._J128Marcador_1_5,
          ...this._appEl_4.mapNestedViews((_ViewJ128Usa1 nestedView) {
            return nestedView._appEl_0.mapNestedViewsWithSingleResult((_ViewJ128Usa2 nestedView) {
              return nestedView._J128Marcador_0_5;
            });
          })
        ];
        this._query_J128Item_0_0_isDirty = false;
      }
      if (this._query_J128Item_5_0_isDirty) {
        this._J128Um_5_5.primeiro = import15.firstOrNull(this._appEl_6.mapNestedViewsWithSingleResult((_ViewJ128Usa3 nestedView) {
          return nestedView._J128Marcador_0_5;
        }));
        this._query_J128Item_5_0_isDirty = false;
      }
      if (this._query_J128Item_7_0_isDirty) {
        this._J128Caixa_7_5.itens = this._appEl_8.mapNestedViewsWithSingleResult((_ViewJ128Usa4 nestedView) {
          return nestedView._J128Marcador_0_5;
        });
        this._query_J128Item_7_0_isDirty = false;
      }
      if (this._query_J128Item_10_0_isDirty) {
        this._J128Lista_10_5.itens = this._appEl_11.mapNestedViewsWithSingleResult((_ViewJ128Usa7 nestedView) {
          return nestedView._J128Marcador_0_5;
        });
        this._query_J128Item_10_0_isDirty = false;
      }
      if (this._viewQuery_r_0_isDirty) {
        _ctx.rs = this._appEl_11.mapNestedViewsWithSingleResult((_ViewJ128Usa7 nestedView) {
          return nestedView._el_0;
        });
        this._viewQuery_r_0_isDirty = false;
      }
    }
    this._compView_7.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_4.destroyNestedViews();
    this._appEl_6.destroyNestedViews();
    this._appEl_8.destroyNestedViews();
    this._appEl_9.destroyNestedViews();
    this._appEl_11.destroyNestedViews();
    this._compView_7.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J128Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J128UsaNgFactory = ComponentFactory<import1.J128Usa>('j128-usa', viewFactory_J128UsaHost0);
ComponentFactory<import1.J128Usa> get J128UsaNgFactory {
  return _J128UsaNgFactory;
}

ComponentFactory<import1.J128Usa> createJ128UsaFactory() {
  return ComponentFactory('j128-usa', viewFactory_J128UsaHost0);
}

class _ViewJ128Usa1 extends import16.EmbeddedView<import1.J128Usa> {
  late final ViewContainer _appEl_0;
  late final import17.NgFor _NgFor_0_9;
  Object? _expr_0;
  _ViewJ128Usa1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _anchor_0 = import11.createAnchor();
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J128Usa2);
    this._NgFor_0_9 = import17.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    this.initRootNode(this._appEl_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.lista;
    if (import14.checkBinding(this._expr_0, currVal_0, 'lista', 'asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1530:1553 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa1(parentView, parentIndex);
}

class _ViewJ128Usa2 extends import16.EmbeddedView<import1.J128Usa> {
  late final import1.J128Marcador _J128Marcador_0_5;
  _ViewJ128Usa2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('b'));
    import11.setAttribute(_el_0, 'j128Marcador', '');
    this._J128Marcador_0_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J128Marcador_0_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && (0 == nodeIndex))) {
      return this._J128Marcador_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ128Usa0>(((this.parentView!).parentView!))._query_J128Item_0_0_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa2(parentView, parentIndex);
}

class _ViewJ128Usa3 extends import16.EmbeddedView<import1.J128Usa> {
  late final import1.J128Marcador _J128Marcador_0_5;
  _ViewJ128Usa3(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('i'));
    import11.setAttribute(_el_0, 'j128Marcador', '');
    this._J128Marcador_0_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J128Marcador_0_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && (0 == nodeIndex))) {
      return this._J128Marcador_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ128Usa0>((this.parentView!))._query_J128Item_5_0_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa3(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa3(parentView, parentIndex);
}

class _ViewJ128Usa4 extends import16.EmbeddedView<import1.J128Usa> {
  late final import1.J128Marcador _J128Marcador_0_5;
  _ViewJ128Usa4(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('em'));
    import11.setAttribute(_el_0, 'j128Marcador', '');
    this._J128Marcador_0_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J128Marcador_0_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && (0 == nodeIndex))) {
      return this._J128Marcador_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ128Usa0>((this.parentView!))._query_J128Item_7_0_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa4(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa4(parentView, parentIndex);
}

class _ViewJ128Usa5 extends import16.EmbeddedView<import1.J128Usa> {
  bool _query_J128Item_1_0_isDirty = true;
  late final import1.J128Lista _J128Lista_1_5;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  _ViewJ128Usa5(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    final _el_1 = import11.appendElement<import6.HtmlElement>(doc, _el_0, 'section');
    import11.setAttribute(_el_1, 'j128Lista', '');
    this._J128Lista_1_5 = import1.J128Lista();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_1, this._J128Lista_1_5);
    }
    final _anchor_2 = import11.appendAnchor(_el_1);
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J128Usa6);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.x);
    }
    this._NgIf_2_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j128_conteudo_dinamico.dart:1729:1738 */;
    this._appEl_2.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._query_J128Item_1_0_isDirty) {
        this._J128Lista_1_5.itens = this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ128Usa6 nestedView) {
          return nestedView._J128Marcador_0_5;
        });
        this._query_J128Item_1_0_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa5(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa5(parentView, parentIndex);
}

class _ViewJ128Usa6 extends import16.EmbeddedView<import1.J128Usa> {
  late final import1.J128Marcador _J128Marcador_0_5;
  _ViewJ128Usa6(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('u'));
    import11.setAttribute(_el_0, 'j128Marcador', '');
    this._J128Marcador_0_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J128Marcador_0_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && (0 == nodeIndex))) {
      return this._J128Marcador_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<_ViewJ128Usa5>((this.parentView!))._query_J128Item_1_0_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa6(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa6(parentView, parentIndex);
}

class _ViewJ128Usa7 extends import16.EmbeddedView<import1.J128Usa> {
  late final import1.J128Marcador _J128Marcador_0_5;
  late final import6.AnchorElement _el_0;
  _ViewJ128Usa7(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    this._el_0 = import5.unsafeCast(doc.createElement('a'));
    import11.setAttribute(this._el_0, 'j128Marcador', '');
    this._J128Marcador_0_5 = import1.J128Marcador();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(this._el_0, this._J128Marcador_0_5);
    }
    this.initRootNode(this._el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J128Item) && (0 == nodeIndex))) {
      return this._J128Marcador_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ128Usa0>((this.parentView!))._query_J128Item_10_0_isDirty = true;
    import5.unsafeCast<ViewJ128Usa0>((this.parentView!))._viewQuery_r_0_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J128Usa7(import18.RenderView parentView, int parentIndex) {
  return _ViewJ128Usa7(parentView, parentIndex);
}

final List<Object> styles$J128UsaHost = const [];

class _ViewJ128UsaHost0 extends import8.HostView<import1.J128Usa> {
  @override
  void build() {
    this.componentView = ViewJ128Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J128Usa();
    this.initRootNode(_el_0);
  }
}

import8.HostView<import1.J128Usa> viewFactory_J128UsaHost0() {
  return _ViewJ128UsaHost0();
}
