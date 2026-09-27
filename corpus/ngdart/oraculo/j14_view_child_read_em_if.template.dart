// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j14_view_child_read_em_if.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j14_view_child_read_em_if.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_if.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import4;
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
import 'package:ngdart/src/core/linker/element_ref.dart';
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'package:ngdart/src/runtime/text_binding.dart' as import19;
import 'dart:core';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import21;

final List<Object> styles$J14ViewChildReadEmIf = const [];

class ViewJ14ViewChildReadEmIf0 extends import0.ComponentView<import1.J14ViewChildReadEmIf> {
  bool _viewQuery_e_0_isDirty = true;
  bool _viewQuery_l_1_isDirty = true;
  late final ViewContainer _appEl_0;
  late final NgIf _NgIf_0_9;
  late final ViewContainer _appEl_1;
  late final import4.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import5.ComponentStyles? _componentStyles;
  ViewJ14ViewChildReadEmIf0(import6.View parentView, int parentIndex) : super(parentView, parentIndex, import7.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import8.unsafeCast(import9.document.createElement('j14-view-child-read-em-if'));
  }
  static String? get _debugComponentUrl {
    return (import8.isDevMode ? 'asset:corpus_ngdart/lib/src/j14_view_child_read_em_if.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final _anchor_0 = import10.appendAnchor(parentRenderNode);
    this._appEl_0 = ViewContainer(0, null, this, _anchor_0);
    var _TemplateRef_0_8 = TemplateRef(this._appEl_0, viewFactory_J14ViewChildReadEmIf1);
    this._NgIf_0_9 = NgIf(this._appEl_0, _TemplateRef_0_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_0, this._NgIf_0_9);
    }
    final _anchor_1 = import10.appendAnchor(parentRenderNode);
    this._appEl_1 = ViewContainer(1, null, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_J14ViewChildReadEmIf2);
    this._NgFor_1_9 = import4.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.recordInput(this._NgIf_0_9, 'ngIf', _ctx.a);
    }
    this._NgIf_0_9.ngIf = _ctx.a /* REF:package:corpus_ngdart/src/j14_view_child_read_em_if.html:5:14 */;
    final currVal_0 = _ctx.itens;
    if (import13.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/j14_view_child_read_em_if.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/j14_view_child_read_em_if.html:41:64 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_0.detectChangesInNestedViews();
    this._appEl_1.detectChangesInNestedViews();
    if ((!import13.debugThrowIfChanged)) {
      if (this._viewQuery_e_0_isDirty) {
        _ctx.elemento = import14.firstOrNull(this._appEl_0.mapNestedViewsWithSingleResult((_ViewJ14ViewChildReadEmIf1 nestedView) {
          return ElementRef(nestedView._el_1);
        }));
        this._viewQuery_e_0_isDirty = false;
      }
      if (this._viewQuery_l_1_isDirty) {
        _ctx.lista = this._appEl_1.mapNestedViewsWithSingleResult((_ViewJ14ViewChildReadEmIf2 nestedView) {
          return ElementRef(nestedView._el_0);
        });
        this._viewQuery_l_1_isDirty = false;
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
      _componentStyles = (styles = import5.ComponentStyles.unscoped(styles$J14ViewChildReadEmIf, _debugComponentUrl));
      if (import8.isDevMode) {
        import5.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J14ViewChildReadEmIfNgFactory = ComponentFactory<import1.J14ViewChildReadEmIf>('j14-view-child-read-em-if', viewFactory_J14ViewChildReadEmIfHost0);
ComponentFactory<import1.J14ViewChildReadEmIf> get J14ViewChildReadEmIfNgFactory {
  return _J14ViewChildReadEmIfNgFactory;
}

ComponentFactory<import1.J14ViewChildReadEmIf> createJ14ViewChildReadEmIfFactory() {
  return ComponentFactory('j14-view-child-read-em-if', viewFactory_J14ViewChildReadEmIfHost0);
}

class _ViewJ14ViewChildReadEmIf1 extends import17.EmbeddedView<import1.J14ViewChildReadEmIf> {
  late final import9.HtmlElement _el_1;
  _ViewJ14ViewChildReadEmIf1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    final _el_0 = import8.unsafeCast(doc.createElement('div'));
    this._el_1 = import10.appendSpan(doc, _el_0);
    final _text_2 = import10.appendText(this._el_1, 'x');
    this.initRootNode(_el_0);
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ14ViewChildReadEmIf0>((this.parentView!))._viewQuery_e_0_isDirty = true;
  }
}

import17.EmbeddedView<void> viewFactory_J14ViewChildReadEmIf1(import18.RenderView parentView, int parentIndex) {
  return _ViewJ14ViewChildReadEmIf1(parentView, parentIndex);
}

class _ViewJ14ViewChildReadEmIf2 extends import17.EmbeddedView<import1.J14ViewChildReadEmIf> {
  final import19.TextBinding _textBinding_1 = import19.TextBinding();
  late final import9.HtmlElement _el_0;
  _ViewJ14ViewChildReadEmIf2(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import9.document;
    this._el_0 = import8.unsafeCast(doc.createElement('p'));
    this._el_0.append(this._textBinding_1.element);
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final local_i = import8.unsafeCast<int>(this.locals['\$implicit']);
    this._textBinding_1.updateTextWithPrimitive(local_i) /* REF:package:corpus_ngdart/src/j14_view_child_read_em_if.html:68:73 */;
  }

  @override
  void dirtyParentQueriesInternal() {
    import8.unsafeCast<ViewJ14ViewChildReadEmIf0>((this.parentView!))._viewQuery_l_1_isDirty = true;
  }
}

import17.EmbeddedView<void> viewFactory_J14ViewChildReadEmIf2(import18.RenderView parentView, int parentIndex) {
  return _ViewJ14ViewChildReadEmIf2(parentView, parentIndex);
}

final List<Object> styles$J14ViewChildReadEmIfHost = const [];

class _ViewJ14ViewChildReadEmIfHost0 extends import21.HostView<import1.J14ViewChildReadEmIf> {
  @override
  void build() {
    this.componentView = ViewJ14ViewChildReadEmIf0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J14ViewChildReadEmIf();
    this.initRootNode(_el_0);
  }
}

import21.HostView<import1.J14ViewChildReadEmIf> viewFactory_J14ViewChildReadEmIfHost0() {
  return _ViewJ14ViewChildReadEmIfHost0();
}
