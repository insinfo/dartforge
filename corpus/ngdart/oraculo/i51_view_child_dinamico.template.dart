// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i51_view_child_dinamico.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i51_view_child_dinamico.dart' as import1;
import 'package:ngdart/src/runtime/text_binding.dart' as import2;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import6;
import 'package:ngdart/src/core/linker/views/view.dart' as import7;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import8;
import 'package:ngdart/src/utilities.dart' as import9;
import 'dart:html' as import10;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import11;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import13;
import 'package:ngdart/src/runtime/check_binding.dart' as import14;
import 'package:ngdart/src/runtime/queries.dart' as import15;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import20;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import21;

final List<Object> styles$I51ViewChildDinamico = const [];

class ViewI51ViewChildDinamico0 extends import0.ComponentView<import1.I51ViewChildDinamico> {
  bool _viewQuery_item_1_isDirty = true;
  bool _viewQuery_campo_2_isDirty = true;
  final import2.TextBinding _textBinding_1 = import2.TextBinding();
  final import2.TextBinding _textBinding_5 = import2.TextBinding();
  late final ViewContainer _appEl_3;
  late final import4.NgFor _NgFor_3_9;
  late final ViewContainer _appEl_6;
  late final NgIf _NgIf_6_9;
  Object? _expr_0;
  static import6.ComponentStyles? _componentStyles;
  ViewI51ViewChildDinamico0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import10.document.createElement('i51-view-child-dinamico'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i51_view_child_dinamico.dart' : null);
  }

  @override
  void build() {
    final _ctx = this.ctx;
    final parentRenderNode = this.initViewRoot();
    final doc = import10.document;
    final _el_0 = import11.appendElement<import10.HtmlElement>(doc, parentRenderNode, 'p');
    _el_0.append(this._textBinding_1.element);
    final _el_2 = import11.appendElement<import10.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_3 = import11.appendAnchor(_el_2);
    this._appEl_3 = ViewContainer(3, 2, this, _anchor_3);
    var _TemplateRef_3_8 = TemplateRef(this._appEl_3, viewFactory_I51ViewChildDinamico1);
    this._NgFor_3_9 = import4.NgFor(this._appEl_3, _TemplateRef_3_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_3, this._NgFor_3_9);
    }
    final _el_4 = import11.appendElement<import10.HtmlElement>(doc, parentRenderNode, 'b');
    _el_4.append(this._textBinding_5.element);
    final _anchor_6 = import11.appendAnchor(parentRenderNode);
    this._appEl_6 = ViewContainer(6, null, this, _anchor_6);
    var _TemplateRef_6_8 = TemplateRef(this._appEl_6, viewFactory_I51ViewChildDinamico2);
    this._NgIf_6_9 = NgIf(this._appEl_6, _TemplateRef_6_8);
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.registerDirective(_anchor_6, this._NgIf_6_9);
    }
    _ctx.fixo = _el_0;
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import14.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i51_view_child_dinamico.html')) {
      if (import13.isDevToolsEnabled) {
        import13.Inspector.instance.recordInput(this._NgFor_3_9, 'ngForOf', currVal_0);
      }
      this._NgFor_3_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i51_view_child_dinamico.html:26:49 */;
      this._expr_0 = currVal_0;
    }
    if ((!import14.debugThrowIfChanged)) {
      this._NgFor_3_9.ngDoCheck();
    }
    if (import13.isDevToolsEnabled) {
      import13.Inspector.instance.recordInput(this._NgIf_6_9, 'ngIf', _ctx.mostrar);
    }
    this._NgIf_6_9.ngIf = _ctx.mostrar /* REF:package:corpus_ngdart/src/i51_view_child_dinamico.html:101:116 */;
    this._appEl_3.detectChangesInNestedViews();
    this._appEl_6.detectChangesInNestedViews();
    if ((!import14.debugThrowIfChanged)) {
      if (this._viewQuery_item_1_isDirty) {
        _ctx.todos = this._appEl_3.mapNestedViewsWithSingleResult((_ViewI51ViewChildDinamico1 nestedView) {
          return nestedView._el_1;
        });
        this._viewQuery_item_1_isDirty = false;
      }
      if (this._viewQuery_campo_2_isDirty) {
        _ctx.campo = import15.firstOrNull(this._appEl_6.mapNestedViewsWithSingleResult((_ViewI51ViewChildDinamico2 nestedView) {
          return nestedView._el_1;
        }));
        this._viewQuery_campo_2_isDirty = false;
      }
    }
    this._textBinding_1.updateTextWithPrimitive(_ctx.n) /* REF:package:corpus_ngdart/src/i51_view_child_dinamico.html:9:14 */;
    this._textBinding_5.updateTextWithPrimitive(_ctx.n) /* REF:package:corpus_ngdart/src/i51_view_child_dinamico.html:87:92 */;
  }

  @override
  void destroyInternal() {
    this._appEl_3.destroyNestedViews();
    this._appEl_6.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I51ViewChildDinamico, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I51ViewChildDinamicoNgFactory = ComponentFactory<import1.I51ViewChildDinamico>('i51-view-child-dinamico', viewFactory_I51ViewChildDinamicoHost0);
ComponentFactory<import1.I51ViewChildDinamico> get I51ViewChildDinamicoNgFactory {
  return _I51ViewChildDinamicoNgFactory;
}

ComponentFactory<import1.I51ViewChildDinamico> createI51ViewChildDinamicoFactory() {
  return ComponentFactory('i51-view-child-dinamico', viewFactory_I51ViewChildDinamicoHost0);
}

class _ViewI51ViewChildDinamico1 extends import17.EmbeddedView<import1.I51ViewChildDinamico> {
  final import2.TextBinding _textBinding_2 = import2.TextBinding();
  late final import10.HtmlElement _el_1;
  _ViewI51ViewChildDinamico1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('li'));
    this._el_1 = import11.appendSpan(doc, _el_0);
    this._el_1.append(this._textBinding_2.element);
    this.initRootNode(_el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import9.unsafeCast<String>(this.locals['\$implicit']);
    this._textBinding_2.updateText(import20.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i51_view_child_dinamico.html:62:67 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import9.unsafeCast<ViewI51ViewChildDinamico0>((this.parentView!))._viewQuery_item_1_isDirty = true;
  }
}

import17.EmbeddedView<void> viewFactory_I51ViewChildDinamico1(import18.RenderView parentView, int parentIndex) {
  return _ViewI51ViewChildDinamico1(parentView, parentIndex);
}

class _ViewI51ViewChildDinamico2 extends import17.EmbeddedView<import1.I51ViewChildDinamico> {
  late final import10.InputElement _el_1;
  _ViewI51ViewChildDinamico2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import10.document;
    final _el_0 = import9.unsafeCast(doc.createElement('div'));
    this._el_1 = import11.appendElement<import10.InputElement>(doc, _el_0, 'input');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import9.unsafeCast<ViewI51ViewChildDinamico0>((this.parentView!))._viewQuery_campo_2_isDirty = true;
  }
}

import17.EmbeddedView<void> viewFactory_I51ViewChildDinamico2(import18.RenderView parentView, int parentIndex) {
  return _ViewI51ViewChildDinamico2(parentView, parentIndex);
}

final List<Object> styles$I51ViewChildDinamicoHost = const [];

class _ViewI51ViewChildDinamicoHost0 extends import21.HostView<import1.I51ViewChildDinamico> {
  @override
  void build() {
    this.componentView = ViewI51ViewChildDinamico0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I51ViewChildDinamico();
    this.initRootNode(_el_0);
  }
}

import21.HostView<import1.I51ViewChildDinamico> viewFactory_I51ViewChildDinamicoHost0() {
  return _ViewI51ViewChildDinamicoHost0();
}
