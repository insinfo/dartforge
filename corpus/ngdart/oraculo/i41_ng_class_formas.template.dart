// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i41_ng_class_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i41_ng_class_formas.dart' as import1;
import 'package:ngdart/src/core/linker/view_container.dart';
import 'package:ngdart/src/common/directives/ng_for.dart' as import3;
import 'package:ngdart/src/common/directives/ng_style.dart' as import4;
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
import 'package:ngdart/src/runtime/text_binding.dart' as import16;
import 'package:ngdart/src/common/directives/ng_class.dart' as import17;
import 'package:ngdart/src/core/linker/views/render_view.dart' as import18;
import 'dart:core';
import 'package:ngdart/src/runtime/interpolate.dart' as import20;
import 'package:ngdart/src/core/linker/views/host_view.dart' as import21;

final List<Object> styles$I41NgClassFormas = const [];

class ViewI41NgClassFormas0 extends import0.ComponentView<import1.I41NgClassFormas> {
  late final ViewContainer _appEl_1;
  late final import3.NgFor _NgFor_1_9;
  late final import4.NgStyle _NgStyle_2_5;
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  late final import5.DivElement _el_2;
  static import6.ComponentStyles? _componentStyles;
  ViewI41NgClassFormas0(import7.View parentView, int parentIndex) : super(parentView, parentIndex, import8.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import9.unsafeCast(import5.document.createElement('i41-ng-class-formas'));
  }
  static String? get _debugComponentUrl {
    return (import9.isDevMode ? 'asset:corpus_ngdart/lib/src/i41_ng_class_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import5.document;
    final _el_0 = import10.appendElement<import5.UListElement>(doc, parentRenderNode, 'ul');
    final _anchor_1 = import10.appendAnchor(_el_0);
    this._appEl_1 = ViewContainer(1, 0, this, _anchor_1);
    var _TemplateRef_1_8 = TemplateRef(this._appEl_1, viewFactory_I41NgClassFormas1);
    this._NgFor_1_9 = import3.NgFor(this._appEl_1, _TemplateRef_1_8);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(_anchor_1, this._NgFor_1_9);
    }
    this._el_2 = import10.appendDiv(doc, parentRenderNode);
    import10.setAttribute(this._el_2, 'style', 'color: red');
    this._NgStyle_2_5 = import4.NgStyle(this._el_2);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(this._el_2, this._NgStyle_2_5);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.itens;
    if (import13.checkBinding(this._expr_0, currVal_0, 'itens', 'package:corpus_ngdart/src/i41_ng_class_formas.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgFor_1_9, 'ngForOf', currVal_0);
      }
      this._NgFor_1_9.ngForOf = currVal_0 /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:8:31 */;
      this._expr_0 = currVal_0;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgFor_1_9.ngDoCheck();
    }
    final currVal_2 = _ctx.estilos;
    if (import13.checkBinding(this._expr_2, currVal_2, 'estilos', 'package:corpus_ngdart/src/i41_ng_class_formas.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgStyle_2_5, 'ngStyle', currVal_2);
      }
      this._NgStyle_2_5.rawStyle = currVal_2 /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:97:116 */;
      this._expr_2 = currVal_2;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgStyle_2_5.ngDoCheck();
    }
    this._appEl_1.detectChangesInNestedViews();
    final currVal_1 = _ctx.x;
    if (import13.checkBinding(this._expr_1, currVal_1, 'x', 'package:corpus_ngdart/src/i41_ng_class_formas.html')) {
      import10.setProperty(this._el_2, 'title', currVal_1) /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:136:147 */;
      this._expr_1 = currVal_1;
    }
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
      _componentStyles = (styles = import6.ComponentStyles.unscoped(styles$I41NgClassFormas, _debugComponentUrl));
      if (import9.isDevMode) {
        import6.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I41NgClassFormasNgFactory = ComponentFactory<import1.I41NgClassFormas>('i41-ng-class-formas', viewFactory_I41NgClassFormasHost0);
ComponentFactory<import1.I41NgClassFormas> get I41NgClassFormasNgFactory {
  return _I41NgClassFormasNgFactory;
}

ComponentFactory<import1.I41NgClassFormas> createI41NgClassFormasFactory() {
  return ComponentFactory('i41-ng-class-formas', viewFactory_I41NgClassFormasHost0);
}

class _ViewI41NgClassFormas1 extends import15.EmbeddedView<import1.I41NgClassFormas> {
  final import16.TextBinding _textBinding_1 = import16.TextBinding();
  late final import17.NgClass _NgClass_0_5;
  Object? _expr_0;
  Object? _expr_2;
  late final import5.HtmlElement _el_0;
  _ViewI41NgClassFormas1(import18.RenderView parentView, int parentIndex) : super(parentView, parentIndex);
  @override
  void build() {
    final doc = import5.document;
    this._el_0 = import9.unsafeCast(doc.createElement('li'));
    this.updateChildClass(this._el_0, 'item');
    this._NgClass_0_5 = import17.NgClass(this._el_0);
    if (import12.isDevToolsEnabled) {
      import12.Inspector.instance.registerDirective(this._el_0, this._NgClass_0_5);
    }
    this._el_0.append(this._textBinding_1.element);
    this.initRootNode(this._el_0);
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final local_x = import9.unsafeCast<String>(this.locals['\$implicit']);
    if (firstCheck) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgClass_0_5, 'class', 'item');
      }
      this._NgClass_0_5.initialClasses = 'item' /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:32:44 */;
    }
    final currVal_2 = local_x;
    if (import13.checkBinding(this._expr_2, currVal_2, 'x', 'package:corpus_ngdart/src/i41_ng_class_formas.html')) {
      if (import12.isDevToolsEnabled) {
        import12.Inspector.instance.recordInput(this._NgClass_0_5, 'ngClass', currVal_2);
      }
      this._NgClass_0_5.rawClass = currVal_2 /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:45:58 */;
      this._expr_2 = currVal_2;
    }
    if ((!import13.debugThrowIfChanged)) {
      this._NgClass_0_5.ngDoCheck();
    }
    final currVal_0 = _ctx.ativo;
    if (import13.checkBinding(this._expr_0, currVal_0, 'ativo', 'package:corpus_ngdart/src/i41_ng_class_formas.html')) {
      import10.updateClassBinding(this._el_0, 'b', currVal_0) /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:59:76 */;
      this._expr_0 = currVal_0;
    }
    this._textBinding_1.updateText(import20.interpolateString0(local_x)) /* REF:package:corpus_ngdart/src/i41_ng_class_formas.html:77:82 */;
  }

  @override
  void destroyInternal() {
    this._NgClass_0_5.ngOnDestroy();
  }
}

import15.EmbeddedView<void> viewFactory_I41NgClassFormas1(import18.RenderView parentView, int parentIndex) {
  return _ViewI41NgClassFormas1(parentView, parentIndex);
}

final List<Object> styles$I41NgClassFormasHost = const [];

class _ViewI41NgClassFormasHost0 extends import21.HostView<import1.I41NgClassFormas> {
  @override
  void build() {
    this.componentView = ViewI41NgClassFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I41NgClassFormas();
    this.initRootNode(_el_0);
  }
}

import21.HostView<import1.I41NgClassFormas> viewFactory_I41NgClassFormasHost0() {
  return _ViewI41NgClassFormasHost0();
}
