// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j138_filho_com_view_providers.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j138_filho_com_view_providers.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/di/errors.dart' as import10;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import12;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J138Arvore = const [];

class ViewJ138Arvore0<T> extends import0.ComponentView<import1.J138Arvore<T>> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ138Arvore0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j138-arvore'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J138Arvore, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J138ArvoreNgFactory = ComponentFactory<import1.J138Arvore>('j138-arvore', viewFactory_J138ArvoreHost0);
ComponentFactory<import1.J138Arvore> get J138ArvoreNgFactory {
  return _J138ArvoreNgFactory;
}

ComponentFactory<import1.J138Arvore<T>> createJ138ArvoreFactory<T>() {
  return ComponentFactory('j138-arvore', viewFactory_J138ArvoreHost0);
}

final List<Object> styles$J138ArvoreHost = const [];

class _ViewJ138ArvoreHost0<T> extends import9.HostView<import1.J138Arvore<T>> {
  @override
  void build() {
    this.componentView = ViewJ138Arvore0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J138Arvore, () {
            return import1.J138Arvore(this.injectorGetOptional(import1.J138Raiz, this.parentIndex));
          })
        : import1.J138Arvore(this.injectorGetOptional(import1.J138Raiz, this.parentIndex)));
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J138Raiz) && (0 == nodeIndex))) {
      return this.component;
    }
    return notFoundResult;
  }
}

import9.HostView<import1.J138Arvore<T>> viewFactory_J138ArvoreHost0<T>() {
  return _ViewJ138ArvoreHost0();
}

final List<Object> styles$J138Item = const [];

class ViewJ138Item0 extends import0.ComponentView<import1.J138Item> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ138Item0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j138-item'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _text_0 = import7.appendText(parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J138Item, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J138ItemNgFactory = ComponentFactory<import1.J138Item>('j138-item', viewFactory_J138ItemHost0);
ComponentFactory<import1.J138Item> get J138ItemNgFactory {
  return _J138ItemNgFactory;
}

ComponentFactory<import1.J138Item> createJ138ItemFactory() {
  return ComponentFactory('j138-item', viewFactory_J138ItemHost0);
}

final List<Object> styles$J138ItemHost = const [];

class _ViewJ138ItemHost0 extends import9.HostView<import1.J138Item> {
  @override
  void build() {
    this.componentView = ViewJ138Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J138Item();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J138Item> viewFactory_J138ItemHost0() {
  return _ViewJ138ItemHost0();
}

final List<Object> styles$J138Grupo = const [];

class ViewJ138Grupo0 extends import0.ComponentView<import1.J138Grupo> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ138Grupo0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j138-grupo'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J138Grupo, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J138GrupoNgFactory = ComponentFactory<import1.J138Grupo>('j138-grupo', viewFactory_J138GrupoHost0);
ComponentFactory<import1.J138Grupo> get J138GrupoNgFactory {
  return _J138GrupoNgFactory;
}

ComponentFactory<import1.J138Grupo> createJ138GrupoFactory() {
  return ComponentFactory('j138-grupo', viewFactory_J138GrupoHost0);
}

final List<Object> styles$J138GrupoHost = const [];

class _ViewJ138GrupoHost0 extends import9.HostView<import1.J138Grupo> {
  @override
  void build() {
    this.componentView = ViewJ138Grupo0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J138Grupo();
    this.component.itens = [];
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J138Grupo> viewFactory_J138GrupoHost0() {
  return _ViewJ138GrupoHost0();
}

final List<Object> styles$J138Caixa = const [];

class ViewJ138Caixa0<T> extends import0.ComponentView<import1.J138Caixa<T>> {
  bool _query_J138Item_1_0_isDirty = true;
  late final ViewJ138Arvore0 _compView_0;
  late final import1.J138Arvore _J138Arvore_0_5;
  late final ViewJ138Grupo0 _compView_1;
  late final import1.J138Grupo _J138Grupo_1_5;
  late final ViewContainer _appEl_2;
  late final import12.NgFor _NgFor_2_9;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ138Caixa0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j138-caixa'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ138Arvore0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J138Arvore_0_5 = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J138Arvore, () {
            return import1.J138Arvore((this.parentView!).injectorGetOptional(import1.J138Raiz, this.parentIndex));
          })
        : import1.J138Arvore((this.parentView!).injectorGetOptional(import1.J138Raiz, this.parentIndex)));
    this._compView_0.create(this._J138Arvore_0_5);
    this._compView_1 = ViewJ138Grupo0(this, 1);
    final _el_1 = this._compView_1.rootElement;
    parentRenderNode.append(_el_1);
    this._J138Grupo_1_5 = import1.J138Grupo();
    final _anchor_2 = import7.createAnchor();
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, (parentView, parentIndex) {
      return viewFactory_J138Caixa1<T>(parentView, parentIndex);
    });
    this._NgFor_2_9 = import12.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
    this._compView_1.createAndProject(this._J138Grupo_1_5, [
      <Object>[this._appEl_2]
    ]);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J138Raiz) && (0 == nodeIndex))) {
      return this._J138Arvore_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.xs;
    if (import15.checkBinding(this._expr_0, currVal_0, 'xs', 'asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_0);
      }
      this._NgFor_2_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j138_filho_com_view_providers.dart:1178:1198 */;
      this._expr_0 = currVal_0;
    }
    if ((!import15.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
    if ((!import15.debugThrowIfChanged)) {
      if (this._query_J138Item_1_0_isDirty) {
        this._J138Grupo_1_5.itens = this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ138Caixa1 nestedView) {
          import3.View.queryChangeDetectorRefs[nestedView._J138Item_0_5] = nestedView._compView_0;
          return nestedView._J138Item_0_5;
        });
        this._query_J138Item_1_0_isDirty = false;
      }
    }
    this._compView_0.detectChanges();
    this._compView_1.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._compView_0.destroyInternalState();
    this._compView_1.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J138Caixa, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J138CaixaNgFactory = ComponentFactory<import1.J138Caixa>('j138-caixa', viewFactory_J138CaixaHost0);
ComponentFactory<import1.J138Caixa> get J138CaixaNgFactory {
  return _J138CaixaNgFactory;
}

ComponentFactory<import1.J138Caixa<T>> createJ138CaixaFactory<T>() {
  return ComponentFactory('j138-caixa', viewFactory_J138CaixaHost0);
}

class _ViewJ138Caixa1<T> extends import16.EmbeddedView<import1.J138Caixa<T>> {
  late final ViewJ138Item0 _compView_0;
  late final import1.J138Item _J138Item_0_5;
  _ViewJ138Caixa1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ138Item0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J138Item_0_5 = import1.J138Item();
    this._compView_0.create(this._J138Item_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ138Caixa0>((this.parentView!))._query_J138Item_1_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import16.EmbeddedView<void> viewFactory_J138Caixa1<T>(import17.RenderView parentView, int parentIndex) {
  return _ViewJ138Caixa1<T>(parentView, parentIndex);
}

final List<Object> styles$J138CaixaHost = const [];

class _ViewJ138CaixaHost0<T> extends import9.HostView<import1.J138Caixa<T>> {
  @override
  void build() {
    this.componentView = ViewJ138Caixa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = (import5.isDevMode
        ? import10.debugInjectorWrap(import1.J138Caixa, () {
            return import1.J138Caixa(this.injectorGet(import1.J138Raiz, this.parentIndex));
          })
        : import1.J138Caixa(this.injectorGet(import1.J138Raiz, this.parentIndex)));
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J138Caixa<T>> viewFactory_J138CaixaHost0<T>() {
  return _ViewJ138CaixaHost0();
}
