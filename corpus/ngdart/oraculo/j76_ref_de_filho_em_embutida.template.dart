// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j76_ref_de_filho_em_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j76_ref_de_filho_em_embutida.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/runtime/interpolate.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import15;
import 'package:ngdart/src/runtime/check_binding.dart' as import16;
import 'package:ngdart/src/runtime/queries.dart' as import17;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import18;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import19;

final List<Object> styles$J76Tabela = const [];

class ViewJ76Tabela0 extends import0.ComponentView<import1.J76Tabela> {
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  static import3.ComponentStyles? _componentStyles;
  ViewJ76Tabela0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j76-tabela'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j76_ref_de_filho_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'i');
    _el_0.append(this._textBinding_1.element);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    this._textBinding_1.updateText(import9.interpolateString0(_ctx.rotulo)) /* REF:asset:corpus_ngdart/lib/src/j76_ref_de_filho_em_embutida.dart:93:103 */;
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J76Tabela, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J76TabelaNgFactory = ComponentFactory<import1.J76Tabela>('j76-tabela', viewFactory_J76TabelaHost0);
ComponentFactory<import1.J76Tabela> get J76TabelaNgFactory {
  return _J76TabelaNgFactory;
}

ComponentFactory<import1.J76Tabela> createJ76TabelaFactory() {
  return ComponentFactory('j76-tabela', viewFactory_J76TabelaHost0);
}

final List<Object> styles$J76TabelaHost = const [];

class _ViewJ76TabelaHost0 extends import11.HostView<import1.J76Tabela> {
  @override
  void build() {
    this.componentView = ViewJ76Tabela0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J76Tabela();
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    bool changed = false;
    if (changed) {
      this.componentView.markAsCheckOnce();
    }
    this.componentView.detectChanges();
  }
}

import11.HostView<import1.J76Tabela> viewFactory_J76TabelaHost0() {
  return _ViewJ76TabelaHost0();
}

final List<Object> styles$J76Cartao = const [];

class ViewJ76Cartao0 extends import0.ComponentView<import1.J76Cartao> {
  static import3.ComponentStyles? _componentStyles;
  ViewJ76Cartao0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j76-cartao'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j76_ref_de_filho_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'b');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J76Cartao, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J76CartaoNgFactory = ComponentFactory<import1.J76Cartao>('j76-cartao', viewFactory_J76CartaoHost0);
ComponentFactory<import1.J76Cartao> get J76CartaoNgFactory {
  return _J76CartaoNgFactory;
}

ComponentFactory<import1.J76Cartao> createJ76CartaoFactory() {
  return ComponentFactory('j76-cartao', viewFactory_J76CartaoHost0);
}

final List<Object> styles$J76CartaoHost = const [];

class _ViewJ76CartaoHost0 extends import11.HostView<import1.J76Cartao> {
  @override
  void build() {
    this.componentView = ViewJ76Cartao0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J76Cartao();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J76Cartao> viewFactory_J76CartaoHost0() {
  return _ViewJ76CartaoHost0();
}

final List<Object> styles$J76RefDeFilhoEmEmbutida = const [];

class ViewJ76RefDeFilhoEmEmbutida0 extends import0.ComponentView<import1.J76RefDeFilhoEmEmbutida> {
  bool _viewQuery_tabela_0_isDirty = true;
  bool _viewQuery_cartao_1_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  static import3.ComponentStyles? _componentStyles;
  ViewJ76RefDeFilhoEmEmbutida0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j76-ref-de-filho-em-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j76_ref_de_filho_em_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import8.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J76RefDeFilhoEmEmbutida1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _anchor_1 = import8.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J76RefDeFilhoEmEmbutida2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j76_ref_de_filho_em_embutida.html:12:27 */;
    if (import15.isDevToolsEnabled) {
      import15.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_1_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j76_ref_de_filho_em_embutida.html:71:86 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    if ((!import16.debugThrowIfChanged)) {
      if (this._viewQuery_tabela_0_isDirty) {
        _ctx.tabela = import17.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ76RefDeFilhoEmEmbutida1 nestedView) {
          import4.View.queryChangeDetectorRefs[nestedView._J76Tabela_0_5] = nestedView._compView_0;
          return nestedView._J76Tabela_0_5;
        }));
        this._viewQuery_tabela_0_isDirty = false;
      }
      if (this._viewQuery_cartao_1_isDirty) {
        _ctx.cartao = import17.firstOrNull(this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ76RefDeFilhoEmEmbutida2 nestedView) {
          return nestedView._J76Cartao_1_5;
        }));
        this._viewQuery_cartao_1_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J76RefDeFilhoEmEmbutida, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J76RefDeFilhoEmEmbutidaNgFactory = ComponentFactory<import1.J76RefDeFilhoEmEmbutida>('j76-ref-de-filho-em-embutida', viewFactory_J76RefDeFilhoEmEmbutidaHost0);
ComponentFactory<import1.J76RefDeFilhoEmEmbutida> get J76RefDeFilhoEmEmbutidaNgFactory {
  return _J76RefDeFilhoEmEmbutidaNgFactory;
}

ComponentFactory<import1.J76RefDeFilhoEmEmbutida> createJ76RefDeFilhoEmEmbutidaFactory() {
  return ComponentFactory('j76-ref-de-filho-em-embutida', viewFactory_J76RefDeFilhoEmEmbutidaHost0);
}

class _ViewJ76RefDeFilhoEmEmbutida1 extends import18.EmbeddedView<import1.J76RefDeFilhoEmEmbutida> {
  late final ViewJ76Tabela0 _compView_0;
  late final import1.J76Tabela _J76Tabela_0_5;
  Object? _expr_0;
  _ViewJ76RefDeFilhoEmEmbutida1(import19.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ76Tabela0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J76Tabela_0_5 = import1.J76Tabela();
    this._compView_0.create(this._J76Tabela_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool changed = false;
    changed = false;
    final currVal_0 = _ctx.nome;
    if (import16.checkBinding(this._expr_0, currVal_0, 'nome', 'package:corpus_ngdart/src/j76_ref_de_filho_em_embutida.html')) {
      if (import15.isDevToolsEnabled) {
        import15.Inspector.instance.recordInput(this._J76Tabela_0_5, 'rotulo', currVal_0);
      }
      this._J76Tabela_0_5.rotulo = currVal_0 /* REF:package:corpus_ngdart/src/j76_ref_de_filho_em_embutida.html:36:51 */;
      changed = true;
      this._expr_0 = currVal_0;
    }
    if (changed) {
      this._compView_0.markAsCheckOnce();
    }
    this._compView_0.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import6.unsafeCast<ViewJ76RefDeFilhoEmEmbutida0>((this.parentView!))._viewQuery_tabela_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import18.EmbeddedView<void> viewFactory_J76RefDeFilhoEmEmbutida1(import19.RenderView parentView, int parentIndex) {
  return _ViewJ76RefDeFilhoEmEmbutida1(parentView, parentIndex);
}

class _ViewJ76RefDeFilhoEmEmbutida2 extends import18.EmbeddedView<import1.J76RefDeFilhoEmEmbutida> {
  late final ViewJ76Cartao0 _compView_1;
  late final import1.J76Cartao _J76Cartao_1_5;
  _ViewJ76RefDeFilhoEmEmbutida2(import19.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import7.document;
    final _el_0 = import6.unsafeCast(doc.createElement('div'));
    this._compView_1 = ViewJ76Cartao0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    _el_0.append(_el_1);
    this._J76Cartao_1_5 = import1.J76Cartao();
    this._compView_1.create(this._J76Cartao_1_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_1.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import6.unsafeCast<ViewJ76RefDeFilhoEmEmbutida0>((this.parentView!))._viewQuery_cartao_1_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_1.destroyInternalState();
  }
}

import18.EmbeddedView<void> viewFactory_J76RefDeFilhoEmEmbutida2(import19.RenderView parentView, int parentIndex) {
  return _ViewJ76RefDeFilhoEmEmbutida2(parentView, parentIndex);
}

final List<Object> styles$J76RefDeFilhoEmEmbutidaHost = const [];

class _ViewJ76RefDeFilhoEmEmbutidaHost0 extends import11.HostView<import1.J76RefDeFilhoEmEmbutida> {
  @override
  void build() {
    this.componentView = ViewJ76RefDeFilhoEmEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J76RefDeFilhoEmEmbutida();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J76RefDeFilhoEmEmbutida> viewFactory_J76RefDeFilhoEmEmbutidaHost0() {
  return _ViewJ76RefDeFilhoEmEmbutidaHost0();
}
