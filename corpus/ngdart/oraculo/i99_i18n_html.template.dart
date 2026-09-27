// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i99_i18n_html.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i99_i18n_html.dart' as import1;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import2;
import 'package:ngdart/src/core/linker/views/view.dart' as import3;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import4;
import 'package:ngdart/src/utilities.dart' as import5;
import 'dart:html' as import6;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import7;
import 'package:ngdart/src/core/linker/app_view_utils.dart' as import8;
import 'package:intl/intl.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$I99I18nHtml = const [];

class ViewI99I18nHtml0 extends import0.ComponentView<import1.I99I18nHtml> {
  static import2.ComponentStyles? _componentStyles;
  ViewI99I18nHtml0(import3.View parentView, int parentIndex) : super(parentView, parentIndex, import4.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import5.unsafeCast(import6.document.createElement('i99-i18n-html'));
  }
  static String? get _debugComponentUrl {
    return (import5.isDevMode ? 'asset:corpus_ngdart/lib/src/i99_i18n_html.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import6.document;
    final _el_0 = import7.appendElement<import6.HtmlElement>(doc, parentRenderNode, 'p');
    final _html_1 = import8.createTrustedHtml(_message_0('<b>', '</b>'));
    _el_0.append(_html_1);
  }

  static String _message_0(String startTag0, String endTag0) {
    return import9.Intl.message('Olá ${startTag0}mundo${endTag0}!', desc: 'saudação', name: 'ViewI99I18nHtml0__message_0', args: [startTag0, endTag0], examples: const {'startTag0': '<b>', 'endTag0': '</b>'});
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import2.ComponentStyles.unscoped(styles$I99I18nHtml, _debugComponentUrl));
      if (import5.isDevMode) {
        import2.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I99I18nHtmlNgFactory = ComponentFactory<import1.I99I18nHtml>('i99-i18n-html', viewFactory_I99I18nHtmlHost0);
ComponentFactory<import1.I99I18nHtml> get I99I18nHtmlNgFactory {
  return _I99I18nHtmlNgFactory;
}

ComponentFactory<import1.I99I18nHtml> createI99I18nHtmlFactory() {
  return ComponentFactory('i99-i18n-html', viewFactory_I99I18nHtmlHost0);
}

final List<Object> styles$I99I18nHtmlHost = const [];

class _ViewI99I18nHtmlHost0 extends import11.HostView<import1.I99I18nHtml> {
  @override
  void build() {
    this.componentView = ViewI99I18nHtml0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I99I18nHtml();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.I99I18nHtml> viewFactory_I99I18nHtmlHost0() {
  return _ViewI99I18nHtmlHost0();
}
