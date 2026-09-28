// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j39_ordem_dos_sujos.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j39_ordem_dos_sujos.dart' as import1;
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
import 'package:ngdart/src/runtime/queries.dart' as import13;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import17;

final List<Object> styles$J39OrdemDosSujos = ['.x._ngcontent-%ID%{color:red}'];

class ViewJ39OrdemDosSujos0 extends import0.ComponentView<import1.J39OrdemDosSujos> {
  bool _viewQuery_botao_1_isDirty = true;
  bool _viewQuery_campo_0_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_1;
  late final NgIf _NgIf_1_9;
  Object? _expr_0;
  late final import4.HtmlElement _el_2;
  static import5.ComponentStyles? _componentStyles;
  ViewJ39OrdemDosSujos0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import4.document.createElement('j39-ordem-dos-sujos'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j39_ordem_dos_sujos.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import9.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J39OrdemDosSujos1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _anchor_1 = import9.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J39OrdemDosSujos2);
    this._NgIf_1_9 = NgIf(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgIf_1_9);
    }
    final doc = import4.document;
    this._el_2 = import9.appendSpan(doc, parentRenderNode);
    this.addShimC(this._el_2);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.b);
    }
    this._NgIf_0_9.ngIf = _ctx.b /* REF:package:corpus_ngdart/src/j39_ordem_dos_sujos.html:5:14 */;
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.recordInput(this._NgIf_1_9, 'ngIf', _ctx.a);
    }
    this._NgIf_1_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j39_ordem_dos_sujos.html:71:80 */;
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    if ((!import12.debugThrowIfChanged)) {
      if (this._viewQuery_campo_0_isDirty) {
        _ctx.campo = import13.firstOrNull(this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ39OrdemDosSujos2 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_campo_0_isDirty = false;
      }
      if (this._viewQuery_botao_1_isDirty) {
        _ctx.botao = import13.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ39OrdemDosSujos1 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_botao_1_isDirty = false;
      }
    }
    final currVal_0 = _ctx.cls;
    if (import12.checkBinding(this._expr_0, currVal_0, 'cls', 'package:corpus_ngdart/src/j39_ordem_dos_sujos.html')) {
      this.updateChildClass(this._el_2, currVal_0) /* REF:package:corpus_ngdart/src/j39_ordem_dos_sujos.html:126:144 */;
      this._expr_0 = currVal_0;
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
      _componentStyles = (styles = import5.ComponentStyles.scoped(styles$J39OrdemDosSujos, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J39OrdemDosSujosNgFactory = ComponentFactory<import1.J39OrdemDosSujos>('j39-ordem-dos-sujos', viewFactory_J39OrdemDosSujosHost0);
ComponentFactory<import1.J39OrdemDosSujos> get J39OrdemDosSujosNgFactory {
  return _J39OrdemDosSujosNgFactory;
}

ComponentFactory<import1.J39OrdemDosSujos> createJ39OrdemDosSujosFactory() {
  return ComponentFactory('j39-ordem-dos-sujos', viewFactory_J39OrdemDosSujosHost0);
}

class _ViewJ39OrdemDosSujos1 extends import15.EmbeddedView<import1.J39OrdemDosSujos> {
  Object? _expr_0;
  late final import4.ButtonElement _el_1;
  _ViewJ39OrdemDosSujos1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this.addShimC(_el_0);
    this._el_1 = import9.appendElement<import4.ButtonElement>(doc, _el_0, 'button');
    this.addShimC(this._el_1);
    final _text_2 = import9.appendText(this._el_1, 'b');
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.cls;
    if (import12.checkBinding(this._expr_0, currVal_0, 'cls', 'package:corpus_ngdart/src/j39_ordem_dos_sujos.html')) {
      this.updateChildClass(this._el_1, currVal_0) /* REF:package:corpus_ngdart/src/j39_ordem_dos_sujos.html:30:48 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ39OrdemDosSujos0>((this.parentView!))._viewQuery_botao_1_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J39OrdemDosSujos1(import16.RenderView parentView, int parentIndex) {
  return _ViewJ39OrdemDosSujos1(parentView, parentIndex);
}

class _ViewJ39OrdemDosSujos2 extends import15.EmbeddedView<import1.J39OrdemDosSujos> {
  Object? _expr_0;
  late final import4.InputElement _el_1;
  _ViewJ39OrdemDosSujos2(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import4.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this.addShimC(_el_0);
    this._el_1 = import9.appendElement<import4.InputElement>(doc, _el_0, 'input');
    this.addShimC(this._el_1);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.cls;
    if (import12.checkBinding(this._expr_0, currVal_0, 'cls', 'package:corpus_ngdart/src/j39_ordem_dos_sujos.html')) {
      this.updateChildClass(this._el_1, currVal_0) /* REF:package:corpus_ngdart/src/j39_ordem_dos_sujos.html:95:112 */;
      this._expr_0 = currVal_0;
    }
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ39OrdemDosSujos0>((this.parentView!))._viewQuery_campo_0_isDirty = true;
  }
}

import15.EmbeddedView<void> viewFactory_J39OrdemDosSujos2(import16.RenderView parentView, int parentIndex) {
  return _ViewJ39OrdemDosSujos2(parentView, parentIndex);
}

final List<Object> styles$J39OrdemDosSujosHost = const [];

class _ViewJ39OrdemDosSujosHost0 extends import17.HostView<import1.J39OrdemDosSujos> {
  @override
  void build() {
    this.componentView = ViewJ39OrdemDosSujos0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J39OrdemDosSujos();
    this.initRootNode(_el_0);
  }
}

import17.HostView<import1.J39OrdemDosSujos> viewFactory_J39OrdemDosSujosHost0() {
  return _ViewJ39OrdemDosSujosHost0();
}
