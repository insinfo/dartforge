// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j131_projetado_e_pedidos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j131_projetado_e_pedidos.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/queries.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J131Alvo = const [];

class ViewJ131Alvo0 extends import0.ComponentView<import1.J131Alvo> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ131Alvo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j131-alvo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _text_0 = import7.appendText(parentRenderNode, 'a');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J131Alvo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J131AlvoNgFactory = ComponentFactory<import1.J131Alvo>('j131-alvo', viewFactory_J131AlvoHost0);
ComponentFactory<import1.J131Alvo> get J131AlvoNgFactory {
  return _J131AlvoNgFactory;
}

ComponentFactory<import1.J131Alvo> createJ131AlvoFactory() {
  return ComponentFactory('j131-alvo', viewFactory_J131AlvoHost0);
}

final List<Object> styles$J131AlvoHost = const [];

class _ViewJ131AlvoHost0 extends import9.HostView<import1.J131Alvo> {
  @override
  void build() {
    this.componentView = ViewJ131Alvo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J131Alvo();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J131Alvo> viewFactory_J131AlvoHost0() {
  return _ViewJ131AlvoHost0();
}

final List<Object> styles$J131Lista = const [];

class ViewJ131Lista0 extends import0.ComponentView<import1.J131Lista> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ131Lista0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j131-lista'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J131Lista, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J131ListaNgFactory = ComponentFactory<import1.J131Lista>('j131-lista', viewFactory_J131ListaHost0);
ComponentFactory<import1.J131Lista> get J131ListaNgFactory {
  return _J131ListaNgFactory;
}

ComponentFactory<import1.J131Lista> createJ131ListaFactory() {
  return ComponentFactory('j131-lista', viewFactory_J131ListaHost0);
}

final List<Object> styles$J131ListaHost = const [];

class _ViewJ131ListaHost0 extends import9.HostView<import1.J131Lista> {
  @override
  void build() {
    this.componentView = ViewJ131Lista0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J131Lista();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J131Lista> viewFactory_J131ListaHost0() {
  return _ViewJ131ListaHost0();
}

final List<Object> styles$J131Caixa = const [];

class ViewJ131Caixa0 extends import0.ComponentView<import1.J131Caixa> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ131Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j131-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J131Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J131CaixaNgFactory = ComponentFactory<import1.J131Caixa>('j131-caixa', viewFactory_J131CaixaHost0);
ComponentFactory<import1.J131Caixa> get J131CaixaNgFactory {
  return _J131CaixaNgFactory;
}

ComponentFactory<import1.J131Caixa> createJ131CaixaFactory() {
  return ComponentFactory('j131-caixa', viewFactory_J131CaixaHost0);
}

final List<Object> styles$J131CaixaHost = const [];

class _ViewJ131CaixaHost0 extends import9.HostView<import1.J131Caixa> {
  late dynamic _J131A_0_6 = import1.fabricaA();
  late dynamic _J131B_0_7 = import1.fabricaB();
  @override
  void build() {
    this.componentView = ViewJ131Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J131Caixa();
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((0 == nodeIndex)) {
      if (identical(token, import1.J131A)) {
        return this._J131A_0_6;
      }
      if (identical(token, import1.J131B)) {
        return this._J131B_0_7;
      }
    }
    return notFoundResult;
  }
}

import9.HostView<import1.J131Caixa> viewFactory_J131CaixaHost0() {
  return _ViewJ131CaixaHost0();
}

final List<Object> styles$J131Usa = const [];

class ViewJ131Usa0 extends import0.ComponentView<import1.J131Usa> {
  bool _viewQuery_J131Alvo_0_isDirty = true;
  late dynamic _J131B_1_6 = import1.fabricaB();
  late dynamic _J131A_1_7 = import1.fabricaA();
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewJ131Caixa0 _compView_1;
  late final import1.J131Caixa _J131Caixa_1_5;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  static import2.ComponentStyles? _componentStyles;
  ViewJ131Usa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j131-usa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import7.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J131Usa1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    this._compView_1 = ViewJ131Caixa0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J131Caixa_1_5 = import1.J131Caixa();
    final _anchor_2 = import7.createAnchor();
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J131Usa2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this._compView_1.createAndProject(this._J131Caixa_1_5, [
      <Object>[this._appEl_2]
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if (((1 <= nodeIndex) && (nodeIndex <= 2))) {
      if (identical(token, import1.J131B)) {
        return this._J131B_1_6;
      }
      if (identical(token, import1.J131A)) {
        return this._J131A_1_7;
      }
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.x);
    }
    this._NgIf_0_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart:1066:1075 */;
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.x);
    }
    this._NgIf_2_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j131_projetado_e_pedidos.dart:1130:1139 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._viewQuery_J131Alvo_0_isDirty) {
        _ctx.alvo = import15.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ131Usa1 nestedView) {
          import3.View.queryChangeDetectorRefs[nestedView._J131Alvo_1_5] = nestedView._compView_1;
          return nestedView._J131Alvo_1_5;
        }));
        this._viewQuery_J131Alvo_0_isDirty = false;
      }
    }
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J131Usa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J131UsaNgFactory = ComponentFactory<import1.J131Usa>('j131-usa', viewFactory_J131UsaHost0);
ComponentFactory<import1.J131Usa> get J131UsaNgFactory {
  return _J131UsaNgFactory;
}

ComponentFactory<import1.J131Usa> createJ131UsaFactory() {
  return ComponentFactory('j131-usa', viewFactory_J131UsaHost0);
}

class _ViewJ131Usa1 extends import16.EmbeddedView<import1.J131Usa> {
  late final ViewJ131Lista0 _compView_0;
  late final import1.J131Lista _J131Lista_0_5;
  late final ViewJ131Alvo0 _compView_1;
  late final import1.J131Alvo _J131Alvo_1_5;
  _ViewJ131Usa1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ131Lista0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J131Lista_0_5 = import1.J131Lista();
    this._compView_1 = ViewJ131Alvo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    this._J131Alvo_1_5 = import1.J131Alvo();
    this._compView_1.create(this._J131Alvo_1_5);
    this._compView_0.createAndProject(this._J131Lista_0_5, [
      <Object>[_el_1]
    ]);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ131Usa0>((this.parentView!))._viewQuery_J131Alvo_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }
}

import16.EmbeddedView<void> viewFactory_J131Usa1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ131Usa1(parentView, parentIndex);
}

class _ViewJ131Usa2 extends import16.EmbeddedView<import1.J131Usa> {
  late final import1.J131Dois _J131Dois_1_5;
  _ViewJ131Usa2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    final _el_1 = import7.appendSpan(doc, _el_0);
    import7.setAttribute(_el_1, 'j131Dois', '');
    this._J131Dois_1_5 = import1.J131Dois(import5.unsafeCast<ViewJ131Usa0>((this.parentView!))._J131B_1_6, import5.unsafeCast<ViewJ131Usa0>((this.parentView!))._J131A_1_7);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_el_1, this._J131Dois_1_5);
    }
    this.initRootNode(_el_0);
  }
}

import16.EmbeddedView<void> viewFactory_J131Usa2(import17.RenderView parentView, int parentIndex) {
  return _ViewJ131Usa2(parentView, parentIndex);
}

final List<Object> styles$J131UsaHost = const [];

class _ViewJ131UsaHost0 extends import9.HostView<import1.J131Usa> {
  @override
  void build() {
    this.componentView = ViewJ131Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J131Usa();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J131Usa> viewFactory_J131UsaHost0() {
  return _ViewJ131UsaHost0();
}
