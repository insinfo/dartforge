// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'j04_i18n_html_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'j04_i18n_html_formas.dart' as import1;
import 'package:intl/intl.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/src/core/linker/app_view_utils.dart' as import9;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import11;

final List<Object> styles$J04I18nHtmlFormas = const [];

class ViewJ04I18nHtmlFormas0 extends import0.ComponentView<import1.J04I18nHtmlFormas> {
  static final String _message_0 = import2.Intl.message('imagem', desc: 'alt');
  static import3.ComponentStyles? _componentStyles;
  ViewJ04I18nHtmlFormas0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('j04-i18n-html-formas'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/j04_i18n_html_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'img');
    import8.setAttribute(_el_0, 'alt', _message_0);
    final _el_1 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _html_2 = import9.createTrustedHtml(_message_1('<b>', '<i>', '</i>', '</b>', '<br>'));
    _el_1.append(_html_2);
    final _el_3 = import8.appendSpan(doc, parentRenderNode);
    final _html_4 = import9.createTrustedHtml(_message_1('<b>', '<i>', '</i>', '</b>', '<br>'));
    _el_3.append(_html_4);
  }

  static String _message_1(String startTag0, String startTag1, String endTag1, String endTag0, String voidElement2) {
    return import2.Intl.message('Diga \'oi\' ${startTag0}a ${startTag1}b${endTag1}${endTag0}${voidElement2}fim \$x', desc: 'd', meaning: 'm', name: 'ViewJ04I18nHtmlFormas0__message_1', args: [startTag0, startTag1, endTag1, endTag0, voidElement2], examples: const {'startTag0': '<b>', 'startTag1': '<i>', 'endTag1': '</i>', 'endTag0': '</b>', 'voidElement2': '<br>'});
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$J04I18nHtmlFormas, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _J04I18nHtmlFormasNgFactory = ComponentFactory<import1.J04I18nHtmlFormas>('j04-i18n-html-formas', viewFactory_J04I18nHtmlFormasHost0);
ComponentFactory<import1.J04I18nHtmlFormas> get J04I18nHtmlFormasNgFactory {
  return _J04I18nHtmlFormasNgFactory;
}

ComponentFactory<import1.J04I18nHtmlFormas> createJ04I18nHtmlFormasFactory() {
  return ComponentFactory('j04-i18n-html-formas', viewFactory_J04I18nHtmlFormasHost0);
}

final List<Object> styles$J04I18nHtmlFormasHost = const [];

class _ViewJ04I18nHtmlFormasHost0 extends import11.HostView<import1.J04I18nHtmlFormas> {
  @override
  void build() {
    this.componentView = ViewJ04I18nHtmlFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.J04I18nHtmlFormas();
    this.initRootNode(_el_0);
  }
}

import11.HostView<import1.J04I18nHtmlFormas> viewFactory_J04I18nHtmlFormasHost0() {
  return _ViewJ04I18nHtmlFormasHost0();
}
