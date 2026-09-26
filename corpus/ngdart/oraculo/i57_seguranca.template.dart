// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i57_seguranca.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i57_seguranca.dart' as import1;
import 'dart:html' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/runtime/check_binding.dart' as import8;
import 'package:ngdart/src/security/safe_html_adapter.dart' as import9;
import 'package:ngdart/src/runtime/interpolate.dart' as import10;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import12;

final List<Object> styles$I57Seguranca = const [];

class ViewI57Seguranca0 extends import0.ComponentView<import1.I57Seguranca> {
  Object? _expr_0;
  Object? _expr_1;
  Object? _expr_2;
  Object? _expr_4;
  Object? _expr_5;
  Object? _expr_6;
  late final import2.AnchorElement _el_0;
  late final import2.HtmlElement _el_2;
  late final import2.IFrameElement _el_3;
  late final import2.AnchorElement _el_4;
  late final import2.DivElement _el_6;
  late final import2.HtmlElement _el_7;
  static import3.ComponentStyles? _componentStyles;
  ViewI57Seguranca0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import2.document.createElement('i57-seguranca'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i57_seguranca.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import2.document;
    this._el_0 = import7.appendElement<import2.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_1 = import7.appendText(this._el_0, 'p');
    this._el_2 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'img');
    this._el_3 = import7.appendElement<import2.IFrameElement>(doc, parentRenderNode, 'iframe');
    this._el_4 = import7.appendElement<import2.AnchorElement>(doc, parentRenderNode, 'a');
    final _text_5 = import7.appendText(this._el_4, 'f');
    this._el_6 = import7.appendDiv(doc, parentRenderNode);
    this._el_7 = import7.appendElement<import2.HtmlElement>(doc, parentRenderNode, 'video');
  }

  @override
  void detectChangesInternal() {
    final _ctx = this.ctx;
    bool firstCheck = this.firstCheck;
    final currVal_0 = _ctx.id;
    if (import8.checkBinding(this._expr_0, currVal_0, '/p/{{id}}', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_0, 'href', import9.sanitizeUrl(import10.interpolate1('/p/', currVal_0, ''))) /* REF:package:corpus_ngdart/src/i57_seguranca.html:3:19 */;
      this._expr_0 = currVal_0;
    }
    final currVal_1 = _ctx.url;
    if (import8.checkBinding(this._expr_1, currVal_1, 'url', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_2, 'src', import9.sanitizeUrl(currVal_1)) /* REF:package:corpus_ngdart/src/i57_seguranca.html:30:41 */;
      this._expr_1 = currVal_1;
    }
    final currVal_2 = _ctx.url;
    if (import8.checkBinding(this._expr_2, currVal_2, 'url', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_3, 'src', import9.sanitizeResourceUrl(currVal_2)) /* REF:package:corpus_ngdart/src/i57_seguranca.html:50:61 */;
      this._expr_2 = currVal_2;
    }
    if (firstCheck) {
      import7.setProperty(this._el_4, 'href', import9.sanitizeUrl('/fixo')) /* REF:package:corpus_ngdart/src/i57_seguranca.html:74:90 */;
    }
    final currVal_4 = _ctx.html;
    if (import8.checkBinding(this._expr_4, currVal_4, 'html', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_6, 'innerHTML', import9.sanitizeHtml(currVal_4)) /* REF:package:corpus_ngdart/src/i57_seguranca.html:101:119 */;
      this._expr_4 = currVal_4;
    }
    final currVal_5 = _ctx.url;
    if (import8.checkBinding(this._expr_5, currVal_5, 'url', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_6, 'title', currVal_5) /* REF:package:corpus_ngdart/src/i57_seguranca.html:120:133 */;
      this._expr_5 = currVal_5;
    }
    final currVal_6 = import10.interpolateString0(_ctx.url);
    if (import8.checkBinding(this._expr_6, currVal_6, '{{url}}', 'package:corpus_ngdart/src/i57_seguranca.html')) {
      import7.setProperty(this._el_7, 'src', import9.sanitizeUrl(currVal_6)) /* REF:package:corpus_ngdart/src/i57_seguranca.html:147:160 */;
      this._expr_6 = currVal_6;
    }
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I57Seguranca, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I57SegurancaNgFactory = ComponentFactory<import1.I57Seguranca>('i57-seguranca', viewFactory_I57SegurancaHost0);
ComponentFactory<import1.I57Seguranca> get I57SegurancaNgFactory {
  return _I57SegurancaNgFactory;
}

ComponentFactory<import1.I57Seguranca> createI57SegurancaFactory() {
  return ComponentFactory('i57-seguranca', viewFactory_I57SegurancaHost0);
}

final List<Object> styles$I57SegurancaHost = const [];

class _ViewI57SegurancaHost0 extends import12.HostView<import1.I57Seguranca> {
  @override
  void build() {
    this.componentView = ViewI57Seguranca0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I57Seguranca();
    this.initRootNode(_el_0);
  }
}

import12.HostView<import1.I57Seguranca> viewFactory_I57SegurancaHost0() {
  return _ViewI57SegurancaHost0();
}
