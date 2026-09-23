// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'a27_on_push.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'a27_on_push.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import4;
import 'package:ngdart/src/core/linker/views/view.dart' as import5;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import6;
import 'package:ngdart/src/utilities.dart' as import7;
import 'dart:html' as import8;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import9;
import 'package:ngdart/src/core/linker/template_ref.dart';
import 'package:ngdart/src/devtools.dart' as import11;
import 'package:ngdart/src/runtime/check_binding.dart' as import12;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/embedded_view.dart' as import14;
import 'package:ngdart/src/runtime/text_binding.dart' as import15;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import16;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import18;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import19;

final List<Object> styles$A27OnPush = const [];

class ViewA27OnPush0 extends import0.ComponentView<import1.A27OnPush> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewA27OnPush0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkOnce) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('a27-on-push'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/a27_on_push.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_A27OnPush1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/a27_on_push.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/a27_on_push.html:8:31 */;
      this._expr_0 = currVal_0;
    }
    if ((!import12.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
  }

  @override
  void destroyInternal() {
    this._appEl_1.destroyNestedViews();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$A27OnPush, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _A27OnPushNgFactory = ComponentFactory<import1.A27OnPush>('a27-on-push', viewFactory_A27OnPushHost0);
ComponentFactory<import1.A27OnPush> get A27OnPushNgFactory {
  return _A27OnPushNgFactory;
}

ComponentFactory<import1.A27OnPush> createA27OnPushFactory() {
  return ComponentFactory('a27-on-push', viewFactory_A27OnPushHost0);
}

class _ViewA27OnPush1 extends import14.EmbeddedView<import1.A27OnPush> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  Object? _expr_0;
  late final import8.HtmlElement _el_0;
  _ViewA27OnPush1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('li'));
    this._el_0.append(this._textBinding_1.element);
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final local_x = import7.unsafeCast<String>(this.locals['\$implicit']);
    final currVal_0 = local_x;
    if (import12.checkBinding(this._expr_0, currVal_0, 'x', 'package:corpus_ngdart/src/a27_on_push.html')) {
      import9.setProperty(this._el_0, 'title', currVal_0) /* REF:package:corpus_ngdart/src/a27_on_push.html:32:43 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import18.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/a27_on_push.html:44:49 */;
  }
}

import14.EmbeddedView<void> viewFactory_A27OnPush1(import16.RenderView parentView, int parentIndex) {
  return _ViewA27OnPush1(parentView, parentIndex);
}

final List<Object> styles$A27OnPushHost = const [];

class _ViewA27OnPushHost0 extends import19.HostView<import1.A27OnPush> {
  @override
  void build() {
    this.componentView = ViewA27OnPush0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.A27OnPush();
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

import19.HostView<import1.A27OnPush> viewFactory_A27OnPushHost0() {
  return _ViewA27OnPushHost0();
}
