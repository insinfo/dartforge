// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i03_ng_for_first_last.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i03_ng_for_first_last.dart' as import1;
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

final List<Object> styles$I03NgForFirstLast = const [];

class ViewI03NgForFirstLast0 extends import0.ComponentView<import1.I03NgForFirstLast> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  Object? _expr_0;
  static import4.ComponentStyles? _componentStyles;
  ViewI03NgForFirstLast0(import5.View parentView, int parentIndex) : super(parentView, parentIndex, import6.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import7.unsafeCast(import8.document.createElement('i03-ng-for-first-last'));
  }
  static String? get _debugComponentUrl {
    return (import7.isDevMode ? 'asset:corpus_ngdart/lib/src/i03_ng_for_first_last.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import8.document;
    final _el_0 = import9.appendElement<import8.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import9.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I03NgForFirstLast1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import11.isDevToolsEnabled) {
      import11.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import12.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i03_ng_for_first_last.html')) {
      if (import11.isDevToolsEnabled) {
        import11.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:8:96 */;
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
      _componentStyles = (styles = import4.ComponentStyles.unscoped(styles$I03NgForFirstLast, _debugComponentUrl));
      if (import7.isDevMode) {
        import4.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I03NgForFirstLastNgFactory = ComponentFactory<import1.I03NgForFirstLast>('i03-ng-for-first-last', viewFactory_I03NgForFirstLastHost0);
ComponentFactory<import1.I03NgForFirstLast> get I03NgForFirstLastNgFactory {
  return _I03NgForFirstLastNgFactory;
}

ComponentFactory<import1.I03NgForFirstLast> createI03NgForFirstLastFactory() {
  return ComponentFactory('i03-ng-for-first-last', viewFactory_I03NgForFirstLastHost0);
}

class _ViewI03NgForFirstLast1 extends import14.EmbeddedView<import1.I03NgForFirstLast> {
  final import15.TextBinding _textBinding_1 = import15.TextBinding();
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_3;
  late final import8.HtmlElement _el_0;
  _ViewI03NgForFirstLast1(import16.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import8.document;
    this._el_0 = import7.unsafeCast(doc.createElement('li'));
    this._el_0.append(this._textBinding_1.element);
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final local_p = import7.unsafeCast<bool>(this.locals['first']);
    final local_u = import7.unsafeCast<bool>(this.locals['last']);
    final local_par = import7.unsafeCast<bool>(this.locals['even']);
    final local_impar = import7.unsafeCast<bool>(this.locals['odd']);
    final local_item = import7.unsafeCast<String>(this.locals['\$implicit']);
    final currVal_0 = local_p;
    if (import12.checkBinding(this._expr_0, currVal_0, 'p', 'package:corpus_ngdart/src/i03_ng_for_first_last.html')) {
      import9.updateClassBinding(this._el_0, 'primeiro', currVal_0) /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:97:117 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = local_u;
    if (import12.checkBinding(this._expr_1, currVal_1, 'u', 'package:corpus_ngdart/src/i03_ng_for_first_last.html')) {
      import9.updateClassBinding(this._el_0, 'ultimo', currVal_1) /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:118:136 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = local_par;
    if (import12.checkBinding(this._expr_2, currVal_2, 'par', 'package:corpus_ngdart/src/i03_ng_for_first_last.html')) {
      import9.updateClassBinding(this._el_0, 'par', currVal_2) /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:137:154 */;
      this._expr_2 = currVal_2;
    }
    final currVal_3 = local_impar;
    if (import12.checkBinding(this._expr_3, currVal_3, 'impar', 'package:corpus_ngdart/src/i03_ng_for_first_last.html')) {
      import9.updateClassBinding(this._el_0, 'impar', currVal_3) /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:155:176 */;
      this._expr_3 = currVal_3;
    }
    this._textBinding_1.updateText(import18.interpolateString0(local_item)) /* REF:package:corpus_ngdart/src/i03_ng_for_first_last.html:177:185 */;
  }
}

import14.EmbeddedView<void> viewFactory_I03NgForFirstLast1(import16.RenderView parentView, int parentIndex) {
  return _ViewI03NgForFirstLast1(parentView, parentIndex);
}

final List<Object> styles$I03NgForFirstLastHost = const [];

class _ViewI03NgForFirstLastHost0 extends import19.HostView<import1.I03NgForFirstLast> {
  @override
  void build() {
    this.componentView = ViewI03NgForFirstLast0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I03NgForFirstLast();
    this.initRootNode(_el_0);
  }
}

import19.HostView<import1.I03NgForFirstLast> viewFactory_I03NgForFirstLastHost0() {
  return _ViewI03NgForFirstLastHost0();
}
