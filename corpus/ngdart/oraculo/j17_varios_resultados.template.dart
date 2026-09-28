// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j17_varios_resultados.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j17_varios_resultados.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
import 'dart:html' as import5;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import10;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import12;
import 'package:ngdart/src/runtime/check_binding.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/runtime/text_binding.dart' as import17;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$J17VariosResultados = const [];

class ViewJ17VariosResultados0 extends import0.ComponentView<import1.J17VariosResultados> {
  bool _viewQuery_s_0_isDirty = true;
  late final ViewContainer _appEl_2;
  late final NgIf _NgIf_2_9;
  late final ViewContainer _appEl_3;
  late final import4.NgFor _NgFor_3_9;
  Object? _expr_0;
  late final import5.HtmlElement _el_0;
  static import6.ComponentStyles? _componentStyles;
  ViewJ17VariosResultados0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('j17-varios-resultados'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/j17_varios_resultados.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    this._el_0 = import10.appendSpan(doc, parentRenderNode);
    final _text_1 = import10.appendText(this._el_0, '0');
    final _anchor_2 = import10.appendAnchor(parentRenderNode);
    this._appEl_2 = ViewContainer(2, null, this, _anchor_2);
    var _TemplateRef_2_8 = TemplateRef(this._appEl_2, viewFactory_J17VariosResultados1);
    this._NgIf_2_9 = NgIf(this._appEl_2, _TemplateRef_2_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_2, this._NgIf_2_9);
    }
    final _anchor_3 = import10.appendAnchor(parentRenderNode);
    this._appEl_3 = ViewContainer(3, null, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_J17VariosResultados2);
    this._NgFor_3_9 = import4.NgFor(this._appEl_3, _TemplateRef_3_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_3, this._NgFor_3_9);
    }
    _ctx.primeiro = this._el_0;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_2_9, 'ngIf', _ctx.a);
    }
    this._NgIf_2_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j17_varios_resultados.html:22:31 */;
    final currVal_0 = _ctx.itens;
    if (import13.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/j17_varios_resultados.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_3_9, 'ngForOf', currVal_0);
      }
      this._NgFor_3_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j17_varios_resultados.html:58:81 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_3_9.ngDoCheck();
    }
    this._appEl_2.detectChangesInNestedViews();
    this._appEl_3.detectChangesInNestedViews();
    if ((!import13.debugThrowIfChanged)) {
      if (this._viewQuery_s_0_isDirty) {
        _ctx.todos = [
          this._el_0,
          ...this._appEl_2.mapNestedViewsWithSingleResult((_ViewJ17VariosResultados1 nestedView) {
            return nestedView._el_1;
          }),
          ...this._appEl_3.mapNestedViewsWithSingleResult((_ViewJ17VariosResultados2 nestedView) {
            return nestedView._el_1;
          })
        ];
        this._viewQuery_s_0_isDirty = false;
      }
    }
  }

  @override
  void destroyInternal() {
    this._appEl_2.destroyNestedViews();
    this._appEl_3.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$J17VariosResultados, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J17VariosResultadosNgFactory = ComponentFactory<import1.J17VariosResultados>('j17-varios-resultados', viewFactory_J17VariosResultadosHost0);
ComponentFactory<import1.J17VariosResultados> get J17VariosResultadosNgFactory {
  return _J17VariosResultadosNgFactory;
}

ComponentFactory<import1.J17VariosResultados> createJ17VariosResultadosFactory() {
  return ComponentFactory('j17-varios-resultados', viewFactory_J17VariosResultadosHost0);
}

class _ViewJ17VariosResultados1 extends import15.EmbeddedView<import1.J17VariosResultados> {
  late final import5.HtmlElement _el_1;
  _ViewJ17VariosResultados1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    this._el_1 = import10.appendSpan(doc, _el_0);
    final _text_2 = import10.appendText(this._el_1, '1');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import9.unsafeCast<ViewJ17VariosResultados0>((this.parentView!))._viewQuery_s_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J17VariosResultados1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ17VariosResultados1(parentView, parentIndex);
}

class _ViewJ17VariosResultados2 extends import15.EmbeddedView<import1.J17VariosResultados> {
  final import17.TextBinding _textBinding_2 = import17.TextBinding();
  late final import5.HtmlElement _el_1;
  _ViewJ17VariosResultados2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    final _el_0 = import9.unsafeCast(doc.createElement('p'));
    this._el_1 = import10.appendSpan(doc, _el_0);
    this._el_1.append(this._textBinding_2.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import9.unsafeCast<int>(this.locals['\$implicit']);
    this._textBinding_2.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/j17_varios_resultados.html:91:96 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import9.unsafeCast<ViewJ17VariosResultados0>((this.parentView!))._viewQuery_s_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J17VariosResultados2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ17VariosResultados2(parentView, parentIndex);
}

final List<Object> styles$J17VariosResultadosHost = const [];

class _ViewJ17VariosResultadosHost0 extends import19.HostView<import1.J17VariosResultados> {
  @override
  void build() {
    this.componentView = ViewJ17VariosResultados0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J17VariosResultados();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.J17VariosResultados> viewFactory_J17VariosResultadosHost0() {
  return _ViewJ17VariosResultadosHost0();
}
