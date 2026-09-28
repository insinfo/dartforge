// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j86_consulta_em_molde.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j86_consulta_em_molde.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import9;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/src/runtime/queries.dart' as import14;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;

final List<Object> styles$J86Item = const [];

class ViewJ86Item0 extends import0.ComponentView<import1.J86Item> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ86Item0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j86-item'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j86_consulta_em_molde.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendDiv(doc, parentRenderNode);
    this.updateChildClass(_el_0, 'corpo');
    this.project(_el_0, 0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J86Item, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J86ItemNgFactory = ComponentFactory<import1.J86Item>('j86-item', viewFactory_J86ItemHost0);
ComponentFactory<import1.J86Item> get J86ItemNgFactory {
  return _J86ItemNgFactory;
}

ComponentFactory<import1.J86Item> createJ86ItemFactory() {
  return ComponentFactory('j86-item', viewFactory_J86ItemHost0);
}

final List<Object> styles$J86ItemHost = const [];

class _ViewJ86ItemHost0 extends import9.HostView<import1.J86Item> {
  @override
  void build() {
    this.componentView = ViewJ86Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J86Item();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J86Item> viewFactory_J86ItemHost0() {
  return _ViewJ86ItemHost0();
}

final List<Object> styles$J86Tabela = const [];

class ViewJ86Tabela0 extends import0.ComponentView<import1.J86Tabela> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ86Tabela0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j86-tabela'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j86_consulta_em_molde.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'i');
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J86Tabela, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J86TabelaNgFactory = ComponentFactory<import1.J86Tabela>('j86-tabela', viewFactory_J86TabelaHost0);
ComponentFactory<import1.J86Tabela> get J86TabelaNgFactory {
  return _J86TabelaNgFactory;
}

ComponentFactory<import1.J86Tabela> createJ86TabelaFactory() {
  return ComponentFactory('j86-tabela', viewFactory_J86TabelaHost0);
}

final List<Object> styles$J86TabelaHost = const [];

class _ViewJ86TabelaHost0 extends import9.HostView<import1.J86Tabela> {
  @override
  void build() {
    this.componentView = ViewJ86Tabela0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J86Tabela();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J86Tabela> viewFactory_J86TabelaHost0() {
  return _ViewJ86TabelaHost0();
}

final List<Object> styles$J86ConsultaEmMolde = const [];

class ViewJ86ConsultaEmMolde0 extends import0.ComponentView<import1.J86ConsultaEmMolde> {
  bool _viewQuery_linha_2_isDirty = true;
  bool _viewQuery_tabela_0_isDirty = true;
  bool _viewQuery_marca_1_isDirty = true;
  late final ViewJ86Item0 _compView_0;
  late final import1.J86Item _J86Item_0_5;
  late final ViewContainer _appEl_1;
  late final import1.J86Corpo _J86Corpo_1_9;
  late final ViewContainer _appEl_2;
  late final import1.J86Corpo _J86Corpo_2_9;
  late final import6.HtmlElement _el_3;
  static import2.ComponentStyles? _componentStyles;
  ViewJ86ConsultaEmMolde0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j86-consulta-em-molde'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j86_consulta_em_molde.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    this._compView_0 = ViewJ86Item0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    parentRenderNode.append(_el_0);
    this._J86Item_0_5 = import1.J86Item();
    final _anchor_1 = import7.createAnchor();
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J86ConsultaEmMolde1);
    this._J86Corpo_1_9 = import1.J86Corpo(_TemplateRef_1_8, this._appEl_1);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._J86Corpo_1_9);
    }
    this._compView_0.createAndProject(this._J86Item_0_5, [
      <Object>[this._appEl_1]
    ]);
    final _anchor_2 = import7.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J86ConsultaEmMolde3);
    this._J86Corpo_2_9 = import1.J86Corpo(_TemplateRef_2_8, this._appEl_2);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._J86Corpo_2_9);
    }
    final doc = import6.document;
    this._el_3 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_4 = import7.appendText(this._el_3, 'tres');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    if (((!import13.debugThrowIfChanged) && firstCheck)) {
      this._J86Corpo_1_9.ngOnInit();
    }
    if (((!import13.debugThrowIfChanged) && firstCheck)) {
      this._J86Corpo_2_9.ngOnInit();
    }
    this._appEl_1.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    if ((!import13.debugThrowIfChanged)) {
      if (this._viewQuery_tabela_0_isDirty) {
        _ctx.tabela = import14.firstOrNull(this._appEl_1.mapNestedViews((_ViewJ86ConsultaEmMolde1 nestedView) {
          return nestedView._appEl_2.mapNestedViewsWithSingleResult((_ViewJ86ConsultaEmMolde2 nestedView) {
            import3.View.queryChangeDetectorRefs[nestedView._J86Tabela_0_5] = nestedView._compView_0;
            return nestedView._J86Tabela_0_5;
          });
        }));
        this._viewQuery_tabela_0_isDirty = false;
      }
      if (this._viewQuery_marca_1_isDirty) {
        _ctx.marca = import14.firstOrNull(this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ86ConsultaEmMolde3 nestedView) {
          return nestedView._el_0;
        }));
        this._viewQuery_marca_1_isDirty = false;
      }
      if (this._viewQuery_linha_2_isDirty) {
        _ctx.linhas = [
          ...this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ86ConsultaEmMolde1 nestedView) {
            return nestedView._el_0;
          }),
          ...this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ86ConsultaEmMolde3 nestedView) {
            return nestedView._el_1;
          }),
          this._el_3
        ];
        this._viewQuery_linha_2_isDirty = false;
      }
    }
    this._compView_0.detectChanges();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
    this._compView_0.destroyInternalState();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J86ConsultaEmMolde, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J86ConsultaEmMoldeNgFactory = ComponentFactory<import1.J86ConsultaEmMolde>('j86-consulta-em-molde', viewFactory_J86ConsultaEmMoldeHost0);
ComponentFactory<import1.J86ConsultaEmMolde> get J86ConsultaEmMoldeNgFactory {
  return _J86ConsultaEmMoldeNgFactory;
}

ComponentFactory<import1.J86ConsultaEmMolde> createJ86ConsultaEmMoldeFactory() {
  return ComponentFactory('j86-consulta-em-molde', viewFactory_J86ConsultaEmMoldeHost0);
}

class _ViewJ86ConsultaEmMolde1 extends import15.EmbeddedView<import1.J86ConsultaEmMolde> {
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final import6.HtmlElement _el_0;
  _ViewJ86ConsultaEmMolde1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    this._el_0 = import5.unsafeCast(doc.createElement('p'));
    final _text_1 = import7.appendText(this._el_0, 'um');
    final _anchor_2 = import7.createAnchor();
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J86ConsultaEmMolde2);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    this.initRootNodesAndSubscriptions(import5.unsafeCast(<Object>[this._el_0, this._appEl_2]), null);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_2_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j86_consulta_em_molde.html:79:94 */;
    this._appEl_2.detectChangesInNestedViews();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ86ConsultaEmMolde0>((this.parentView!))._viewQuery_linha_2_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
  }
}

import15.EmbeddedView<void> viewFactory_J86ConsultaEmMolde1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ86ConsultaEmMolde1(parentView, parentIndex);
}

class _ViewJ86ConsultaEmMolde2 extends import15.EmbeddedView<import1.J86ConsultaEmMolde> {
  late final ViewJ86Tabela0 _compView_0;
  late final import1.J86Tabela _J86Tabela_0_5;
  _ViewJ86ConsultaEmMolde2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    this._compView_0 = ViewJ86Tabela0(this, 0);
    final _el_0 = this._compView_0.rootElement;
    this._J86Tabela_0_5 = import1.J86Tabela();
    this._compView_0.create(this._J86Tabela_0_5);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    this._compView_0.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ86ConsultaEmMolde0>(((this.parentView!).parentView!))._viewQuery_tabela_0_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_0.destroyInternalState();
  }
}

import15.EmbeddedView<void> viewFactory_J86ConsultaEmMolde2(import17.RenderView parentView, int parentIndex) {
  return _ViewJ86ConsultaEmMolde2(parentView, parentIndex);
}

class _ViewJ86ConsultaEmMolde3 extends import15.EmbeddedView<import1.J86ConsultaEmMolde> {
  late final import6.HtmlElement _el_0;
  late final import6.HtmlElement _el_1;
  _ViewJ86ConsultaEmMolde3(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    this._el_0 = import5.unsafeCast(doc.createElement('span'));
    this._el_1 = import5.unsafeCast(doc.createElement('p'));
    final _text_2 = import7.appendText(this._el_1, 'dois');
    this.initRootNodesAndSubscriptions(import5.unsafeCast(<Object>[this._el_0, this._el_1]), null);
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ86ConsultaEmMolde0>((this.parentView!))._viewQuery_marca_1_isDirty = true;
    import5.unsafeCast<ViewJ86ConsultaEmMolde0>((this.parentView!))._viewQuery_linha_2_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J86ConsultaEmMolde3(import17.RenderView parentView, int parentIndex) {
  return _ViewJ86ConsultaEmMolde3(parentView, parentIndex);
}

final List<Object> styles$J86ConsultaEmMoldeHost = const [];

class _ViewJ86ConsultaEmMoldeHost0 extends import9.HostView<import1.J86ConsultaEmMolde> {
  @override
  void build() {
    this.componentView = ViewJ86ConsultaEmMolde0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J86ConsultaEmMolde();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J86ConsultaEmMolde> viewFactory_J86ConsultaEmMoldeHost0() {
  return _ViewJ86ConsultaEmMoldeHost0();
}
