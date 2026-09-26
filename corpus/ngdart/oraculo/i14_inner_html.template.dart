// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i14_inner_html.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i14_inner_html.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/src/security/safe_html_adapter.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I14InnerHtml = const [];

class ViewI14InnerHtml0 extends import0.ComponentView<import1.I14InnerHtml> {
  Object? _expr_0;
  Object? _expr_1;
  late final import2.DivElement _el_0;
  late final import2.AnchorElement _el_1;
  static import3.ComponentStyles? _componentStyles;
  ViewI14InnerHtml0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i14-inner-html'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i14_inner_html.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendDiv(doc, parentRenderNode);
    this._el_1 = import7.appendElement<import2.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_2 = import7.appendText(this._el_1, 'x');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    final currVal_0 = _ctx.html;
    if (import8.checkBinding(this._expr_0, currVal_0, 'html', 'package:corpus_ngdart/src/i14_inner_html.html')) {
      import7.setProperty(this._el_0, 'innerHTML', import9.sanitizeHtml(currVal_0)) /* REF:package:corpus_ngdart/src/i14_inner_html.html:5:23 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.url;
    if (import8.checkBinding(this._expr_1, currVal_1, 'url', 'package:corpus_ngdart/src/i14_inner_html.html')) {
      import7.setProperty(this._el_1, 'href', import9.sanitizeUrl(currVal_1)) /* REF:package:corpus_ngdart/src/i14_inner_html.html:33:45 */;
      this._expr_1 = currVal_1;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I14InnerHtml, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I14InnerHtmlNgFactory = ComponentFactory<import1.I14InnerHtml>('i14-inner-html', viewFactory_I14InnerHtmlHost0);
ComponentFactory<import1.I14InnerHtml> get I14InnerHtmlNgFactory {
  return _I14InnerHtmlNgFactory;
}

ComponentFactory<import1.I14InnerHtml> createI14InnerHtmlFactory() {
  return ComponentFactory('i14-inner-html', viewFactory_I14InnerHtmlHost0);
}

final List<Object> styles$I14InnerHtmlHost = const [];

class _ViewI14InnerHtmlHost0 extends import11.HostView<import1.I14InnerHtml> {
  @override
  void build() {
    this.componentView = ViewI14InnerHtml0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I14InnerHtml();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I14InnerHtml> viewFactory_I14InnerHtmlHost0() {
  return _ViewI14InnerHtmlHost0();
}
