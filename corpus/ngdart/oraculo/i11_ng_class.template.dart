// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i11_ng_class.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i11_ng_class.dart' as import1;
import 'package:ngdart/src/common/directives/ng_class.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/devtools.dart' as import9;
import 'package:ngdart/src/runtime/check_binding.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I11NgClass = const [];

class ViewI11NgClass0 extends import0.ComponentView<import1.I11NgClass> {
  late final import2.NgClass _NgClass_0_5;
  late final import2.NgClass _NgClass_1_5;
  Object? _expr_0;
  Object? _expr_1;
  static import3.ComponentStyles? _componentStyles;
  ViewI11NgClass0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i11-ng-class'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i11_ng_class.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendDiv(doc, parentRenderNode);
    this._NgClass_0_5 = import2.NgClass(_el_0);
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_0, this._NgClass_0_5);
    }
    final _el_1 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    this._NgClass_1_5 = import2.NgClass(_el_1);
    if (import9.isDevToolsEnabled) {
      import9.Inspector.instance.registerDirective(_el_1, this._NgClass_1_5);
    }
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.classes;
    if (import10.checkBinding(this._expr_0, currVal_0, 'classes', 'package:corpus_ngdart/src/i11_ng_class.html')) {
      if (import9.isDevToolsEnabled) {
        import9.Inspector.instance.recordInput(this._NgClass_0_5, 'ngClass', currVal_0);
      }
      this._NgClass_0_5.rawClass = currVal_0 /* REF:package:corpus_ngdart/src/i11_ng_class.html:5:24 */;
      this._expr_0 = currVal_0;
    }
    if ((!import10.debugThrowIfChanged)) {
      this._NgClass_0_5.ngDoCheck();
    }
    final currVal_1 = _ctx.lista;
    if (import10.checkBinding(this._expr_1, currVal_1, 'lista', 'package:corpus_ngdart/src/i11_ng_class.html')) {
      if (import9.isDevToolsEnabled) {
        import9.Inspector.instance.recordInput(this._NgClass_1_5, 'ngClass', currVal_1);
      }
      this._NgClass_1_5.rawClass = currVal_1 /* REF:package:corpus_ngdart/src/i11_ng_class.html:34:51 */;
      this._expr_1 = currVal_1;
    }
    if ((!import10.debugThrowIfChanged)) {
      this._NgClass_1_5.ngDoCheck();
    }
  }

  @override
  void destroyInternal() {
    this._NgClass_0_5.ngOnDestroy();
    this._NgClass_1_5.ngOnDestroy();
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I11NgClass, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I11NgClassNgFactory = ComponentFactory<import1.I11NgClass>('i11-ng-class', viewFactory_I11NgClassHost0);
ComponentFactory<import1.I11NgClass> get I11NgClassNgFactory {
  return _I11NgClassNgFactory;
}

ComponentFactory<import1.I11NgClass> createI11NgClassFactory() {
  return ComponentFactory('i11-ng-class', viewFactory_I11NgClassHost0);
}

final List<Object> styles$I11NgClassHost = const [];

class _ViewI11NgClassHost0 extends import12.HostView<import1.I11NgClass> {
  @override
  void build() {
    this.componentView = ViewI11NgClass0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I11NgClass();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I11NgClass> viewFactory_I11NgClassHost0() {
  return _ViewI11NgClassHost0();
}
