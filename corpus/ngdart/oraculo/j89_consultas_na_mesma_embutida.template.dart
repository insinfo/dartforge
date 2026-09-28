// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j89_consultas_na_mesma_embutida.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j89_consultas_na_mesma_embutida.dart' as import1;
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
import 'package:ngdart/src/common/directives/ng_for.dart' as import12;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import14;
import 'package:ngdart/src/runtime/check_binding.dart' as import15;
import 'package:ngdart/src/runtime/queries.dart' as import16;
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/runtime/text_binding.dart' as import19;
import 'dart:core';

final List<Object> styles$J89Item = const [];

class ViewJ89Item0 extends import0.ComponentView<import1.J89Item> {
  static import2.ComponentStyles? _componentStyles;
  ViewJ89Item0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j89-item'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j89_consultas_na_mesma_embutida.dart' : null);
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
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J89Item, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J89ItemNgFactory = ComponentFactory<import1.J89Item>('j89-item', viewFactory_J89ItemHost0);
ComponentFactory<import1.J89Item> get J89ItemNgFactory {
  return _J89ItemNgFactory;
}

ComponentFactory<import1.J89Item> createJ89ItemFactory() {
  return ComponentFactory('j89-item', viewFactory_J89ItemHost0);
}

final List<Object> styles$J89ItemHost = const [];

class _ViewJ89ItemHost0 extends import9.HostView<import1.J89Item> {
  @override
  void build() {
    this.componentView = ViewJ89Item0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J89Item();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J89Item> viewFactory_J89ItemHost0() {
  return _ViewJ89ItemHost0();
}

final List<Object> styles$J89ConsultasNaMesmaEmbutida = const [];

class ViewJ89ConsultasNaMesmaEmbutida0 extends import0.ComponentView<import1.J89ConsultasNaMesmaEmbutida> {
  bool _viewQuery_linha_0_isDirty = true;
  bool _viewQuery_J89Item_1_isDirty = true;
  bool _viewQuery_marca_2_isDirty = true;
  bool _viewQuery_marca_3_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_2;
  late final import12.NgFor _NgFor_2_9;
  Object? _expr_0;
  static import2.ComponentStyles? _componentStyles;
  ViewJ89ConsultasNaMesmaEmbutida0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('j89-consultas-na-mesma-embutida'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/j89_consultas_na_mesma_embutida.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import7.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J89ConsultasNaMesmaEmbutida1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final doc = import6.document;
    final _el_1 = import7.appendElement<import6.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_2 = import7.appendAnchor(_el_1);
    this._appEl_2 = ViewContainer(2, 1, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J89ConsultasNaMesmaEmbutida2);
    this._NgFor_2_9 = import12.NgFor(this._appEl_2, _TemplateRef_2_8);
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_anchor_2, this._NgFor_2_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_0_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/j89_consultas_na_mesma_embutida.html:5:20 */;
    final currVal_0 = _ctx.itens;
    if (import15.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/j89_consultas_na_mesma_embutida.html')) {
      if (import14.isDevToolsEnabled) {
        import14.Inspector.instance.recordInput(this._NgFor_2_9, 'ngForOf', currVal_0);
      }
      this._NgFor_2_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j89_consultas_na_mesma_embutida.html:157:180 */;
      this._expr_0 = currVal_0;
    }
    if ((!import15.debugThrowIfChanged)) {
      this._NgFor_2_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_2.detectChangesInNestedViews();
    if ((!import15.debugThrowIfChanged)) {
      if (this._viewQuery_linha_0_isDirty) {
        _ctx.linhas = [
          ...this._appEl_0.mapNestedViews((_ViewJ89ConsultasNaMesmaEmbutida1 nestedView) {
            return [nestedView._el_1, nestedView._el_4];
          }),
          ...this._appEl_2.mapNestedViews((_ViewJ89ConsultasNaMesmaEmbutida2 nestedView) {
            return [nestedView._el_1, nestedView._el_4];
          })
        ];
        this._viewQuery_linha_0_isDirty = false;
      }
      if (this._viewQuery_J89Item_1_isDirty) {
        _ctx.todos = this._appEl_0.mapNestedViews((_ViewJ89ConsultasNaMesmaEmbutida1 nestedView) {
          return [nestedView._J89Item_3_5, nestedView._J89Item_6_5];
        });
        this._viewQuery_J89Item_1_isDirty = false;
      }
      if (this._viewQuery_marca_2_isDirty) {
        _ctx.servico = import16.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ89ConsultasNaMesmaEmbutida1 nestedView) {
          return nestedView._J89Servico_7_6;
        }));
        this._viewQuery_marca_2_isDirty = false;
      }
      if (this._viewQuery_marca_3_isDirty) {
        _ctx.marcas = this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ89ConsultasNaMesmaEmbutida1 nestedView) {
          return nestedView._J89Marca_7_5;
        });
        this._viewQuery_marca_3_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
    this._appEl_2.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$J89ConsultasNaMesmaEmbutida, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J89ConsultasNaMesmaEmbutidaNgFactory = ComponentFactory<import1.J89ConsultasNaMesmaEmbutida>('j89-consultas-na-mesma-embutida', viewFactory_J89ConsultasNaMesmaEmbutidaHost0);
ComponentFactory<import1.J89ConsultasNaMesmaEmbutida> get J89ConsultasNaMesmaEmbutidaNgFactory {
  return _J89ConsultasNaMesmaEmbutidaNgFactory;
}

ComponentFactory<import1.J89ConsultasNaMesmaEmbutida> createJ89ConsultasNaMesmaEmbutidaFactory() {
  return ComponentFactory('j89-consultas-na-mesma-embutida', viewFactory_J89ConsultasNaMesmaEmbutidaHost0);
}

class _ViewJ89ConsultasNaMesmaEmbutida1 extends import17.EmbeddedView<import1.J89ConsultasNaMesmaEmbutida> {
  late final ViewJ89Item0 _compView_3;
  late final import1.J89Item _J89Item_3_5;
  late final ViewJ89Item0 _compView_6;
  late final import1.J89Item _J89Item_6_5;
  late final import1.J89Marca _J89Marca_7_5;
  late final import1.J89Servico _J89Servico_7_6;
  late final import6.HtmlElement _el_1;
  late final import6.HtmlElement _el_4;
  _ViewJ89ConsultasNaMesmaEmbutida1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('div'));
    this._el_1 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'p');
    final _text_2 = import7.appendText(this._el_1, 'a');
    this._compView_3 = ViewJ89Item0(this, 3);
    final _el_3 = this._compView_3.rootElement;
    _el_0.append(_el_3);
    this._J89Item_3_5 = import1.J89Item();
    this._compView_3.create(this._J89Item_3_5);
    this._el_4 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'p');
    final _text_5 = import7.appendText(this._el_4, 'b');
    this._compView_6 = ViewJ89Item0(this, 6);
    final _el_6 = this._compView_6.rootElement;
    _el_0.append(_el_6);
    this._J89Item_6_5 = import1.J89Item();
    this._compView_6.create(this._J89Item_6_5);
    final _el_7 = import7.appendSpan(doc, _el_0);
    import7.setAttribute(_el_7, 'j89-marca', '');
    this._J89Marca_7_5 = import1.J89Marca();
    this._J89Servico_7_6 = import1.J89Servico();
    if (import14.isDevToolsEnabled) {
      import14.Inspector.instance.registerDirective(_el_7, this._J89Marca_7_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J89Servico) && (7 == nodeIndex))) {
      return this._J89Servico_7_6;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    this._compView_3.detectChanges();
    this._compView_6.detectChanges();
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ89ConsultasNaMesmaEmbutida0>((this.parentView!))._viewQuery_linha_0_isDirty = true;
    import5.unsafeCast<ViewJ89ConsultasNaMesmaEmbutida0>((this.parentView!))._viewQuery_J89Item_1_isDirty = true;
    import5.unsafeCast<ViewJ89ConsultasNaMesmaEmbutida0>((this.parentView!))._viewQuery_marca_2_isDirty = true;
    import5.unsafeCast<ViewJ89ConsultasNaMesmaEmbutida0>((this.parentView!))._viewQuery_marca_3_isDirty = true;
  }

  @override
  void destroyInternal() {
    this._compView_3.destroyInternalState();
    this._compView_6.destroyInternalState();
  }
}

import17.EmbeddedView<void> viewFactory_J89ConsultasNaMesmaEmbutida1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ89ConsultasNaMesmaEmbutida1(parentView, parentIndex);
}

class _ViewJ89ConsultasNaMesmaEmbutida2 extends import17.EmbeddedView<import1.J89ConsultasNaMesmaEmbutida> {
  final import19.TextBinding _textBinding_2 = import19.TextBinding();
  late final import6.HtmlElement _el_1;
  late final import6.HtmlElement _el_4;
  _ViewJ89ConsultasNaMesmaEmbutida2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import6.document;
    final _el_0 = import5.unsafeCast(doc.createElement('li'));
    this._el_1 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'b');
    this._el_1.append(this._textBinding_2.element);
    final _text_3 = import7.appendText(_el_0, ' ');
    this._el_4 = import7.appendElement<import6.HtmlElement>(doc, _el_0, 'em');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import5.unsafeCast<int>(this.locals['\$implicit']);
    this._textBinding_2.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/j89_consultas_na_mesma_embutida.html:196:203 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import5.unsafeCast<ViewJ89ConsultasNaMesmaEmbutida0>((this.parentView!))._viewQuery_linha_0_isDirty = true;
  }
}

import17.EmbeddedView<void> viewFactory_J89ConsultasNaMesmaEmbutida2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ89ConsultasNaMesmaEmbutida2(parentView, parentIndex);
}

final List<Object> styles$J89ConsultasNaMesmaEmbutidaHost = const [];

class _ViewJ89ConsultasNaMesmaEmbutidaHost0 extends import9.HostView<import1.J89ConsultasNaMesmaEmbutida> {
  @override
  void build() {
    this.componentView = ViewJ89ConsultasNaMesmaEmbutida0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J89ConsultasNaMesmaEmbutida();
    this.initRootNode(_el_0);
  }
}

import9.HostView<import1.J89ConsultasNaMesmaEmbutida> viewFactory_J89ConsultasNaMesmaEmbutidaHost0() {
  return _ViewJ89ConsultasNaMesmaEmbutidaHost0();
}
