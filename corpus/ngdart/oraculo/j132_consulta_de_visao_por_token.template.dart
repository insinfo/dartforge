// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j132_consulta_de_visao_por_token.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j132_consulta_de_visao_por_token.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import5;
import 'package:ngdart/src/core/linker/views/view.dart' as import6;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import7;
import 'package:ngdart/src/utilities.dart' as import8;
import 'dart:html' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/src/runtime/queries.dart' as import14;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/runtime/text_binding.dart' as import19;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import20;

final List<Object> styles$J132Usa = const [];

class ViewJ132Usa0 extends import0.ComponentView<import1.J132Usa> {
  bool _viewQuery_J132Item_0_isDirty = true;
  bool _viewQuery_J132Item_1_isDirty = true;
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewJ132Usa0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('j132-usa'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J132Usa1);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    final _anchor_1 = import10.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J132Usa4);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.grupos;
    if (import13.checkBinding(this._expr_0, currVal_0, 'grupos', 'asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart:598:622 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.x);
    }
    this._NgIf_1_9.ngIf = _ctx.x /* REF:asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart:725:734 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    if ((!import13.debugThrowIfChanged)) {
      if (this._viewQuery_J132Item_0_isDirty) {
        _ctx.itens = [
          ...this._appEl_0.mapNestedViews((_ViewJ132Usa1 nestedView) {
            return nestedView._appEl_1.mapNestedViews((_ViewJ132Usa2 nestedView) {
              return nestedView._appEl_0.mapNestedViewsWithSingleResult((_ViewJ132Usa3 nestedView) {
                return nestedView._J132Marca_0_5;
              });
            });
          }),
          ...this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ132Usa4 nestedView) {
            return nestedView._J132Marca_0_5;
          })
        ];
        this._viewQuery_J132Item_0_isDirty = false;
      }
      if (this._viewQuery_J132Item_1_isDirty) {
        _ctx.primeiro = import14.firstOrNull([
          ...this._appEl_0.mapNestedViews((_ViewJ132Usa1 nestedView) {
            return nestedView._appEl_1.mapNestedViews((_ViewJ132Usa2 nestedView) {
              return nestedView._appEl_0.mapNestedViewsWithSingleResult((_ViewJ132Usa3 nestedView) {
                return nestedView._J132Marca_0_5;
              });
            });
          }),
          ...this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ132Usa4 nestedView) {
            return nestedView._J132Marca_0_5;
          })
        ]);
        this._viewQuery_J132Item_1_isDirty = false;
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J132Usa, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J132UsaNgFactory = ComponentFactory<import1.J132Usa>('j132-usa', viewFactory_J132UsaHost0);
ComponentFactory<import1.J132Usa> get J132UsaNgFactory {
  return _J132UsaNgFactory;
}

ComponentFactory<import1.J132Usa> createJ132UsaFactory() {
  return ComponentFactory('j132-usa', viewFactory_J132UsaHost0);
}

class _ViewJ132Usa1 extends import16.EmbeddedView<import1.J132Usa> {
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  _ViewJ132Usa1(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J132Usa2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_g = import8.unsafeCast<int>(this.locals['\$implicit']);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', (local_g > 0));
    }
    this._NgIf_1_9.ngIf = (local_g > 0) /* REF:asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart:636:650 */;
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }
}

import16.EmbeddedView<void> viewFactory_J132Usa1(import17.RenderView parentView, int parentIndex) {
  return _ViewJ132Usa1(parentView, parentIndex);
}

class _ViewJ132Usa2 extends import16.EmbeddedView<import1.J132Usa> {
  late final ViewContainer _appEl_0;
  late final import3.NgFor _NgFor_0_9;
  Object? _expr_0;
  _ViewJ132Usa2(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final _anchor_0 = import10.createAnchor();
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J132Usa3);
    this._NgFor_0_9 = import3.NgFor(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgFor_0_9);
    }
    this.initRootNode(this._appEl_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.lista;
    if (import13.checkBinding(this._expr_0, currVal_0, 'lista', 'asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_0_9, 'ngForOf', currVal_0);
      }
      this._NgFor_0_9.ngForOf = currVal_0 /* REF:asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart:657:680 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_0_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_0.destroyNestedViews();
  }
}

import16.EmbeddedView<void> viewFactory_J132Usa2(import17.RenderView parentView, int parentIndex) {
  return _ViewJ132Usa2(parentView, parentIndex);
}

class _ViewJ132Usa3 extends import16.EmbeddedView<import1.J132Usa> {
  final import19.TextBinding _textBinding_1 = import19.TextBinding();
  late final import1.J132Marca _J132Marca_0_5;
  _ViewJ132Usa3(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('span'));
    import10.setAttribute(_el_0, 'j132Marca', '');
    this._J132Marca_0_5 = import1.J132Marca();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J132Marca_0_5);
    }
    _el_0.append(this._textBinding_1.element);
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J132Item) && (nodeIndex <= 1))) {
      return this._J132Marca_0_5;
    }
    return notFoundResult;
  }

  @override
  void detectChangesInternal() {
    final local_i = import8.unsafeCast<int>(this.locals['\$implicit']);
    this._textBinding_1.updateTextWithPrimitive(local_i) /* REF:asset:corpus_ngdart/lib/src/j132_consulta_de_visao_por_token.dart:691:696 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ132Usa0>((((this.parentView!).parentView!).parentView!))._viewQuery_J132Item_0_isDirty = true;
    import8.unsafeCast<ViewJ132Usa0>((((this.parentView!).parentView!).parentView!))._viewQuery_J132Item_1_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J132Usa3(import17.RenderView parentView, int parentIndex) {
  return _ViewJ132Usa3(parentView, parentIndex);
}

class _ViewJ132Usa4 extends import16.EmbeddedView<import1.J132Usa> {
  late final import1.J132Marca _J132Marca_0_5;
  _ViewJ132Usa4(import17.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('p'));
    import10.setAttribute(_el_0, 'j132Marca', '');
    this._J132Marca_0_5 = import1.J132Marca();
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_el_0, this._J132Marca_0_5);
    }
    this.initRootNode(_el_0);
  }

  @override
  dynamic injectorGetInternal(dynamic token, int nodeIndex, dynamic notFoundResult) {
    if ((identical(token, import1.J132Item) && (0 == nodeIndex))) {
      return this._J132Marca_0_5;
    }
    return notFoundResult;
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ132Usa0>((this.parentView!))._viewQuery_J132Item_0_isDirty = true;
    import8.unsafeCast<ViewJ132Usa0>((this.parentView!))._viewQuery_J132Item_1_isDirty = true;
  }
}

import16.EmbeddedView<void> viewFactory_J132Usa4(import17.RenderView parentView, int parentIndex) {
  return _ViewJ132Usa4(parentView, parentIndex);
}

final List<Object> styles$J132UsaHost = const [];

class _ViewJ132UsaHost0 extends import20.HostView<import1.J132Usa> {
  @override
  void build() {
    this.componentView = ViewJ132Usa0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J132Usa();
    this.initRootNode(_el_0);
  }
}

import20.HostView<import1.J132Usa> viewFactory_J132UsaHost0() {
  return _ViewJ132UsaHost0();
}
