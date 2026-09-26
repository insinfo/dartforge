// **************************************************************************
// Generator: AngularDart Compiler
// **************************************************************************

import 'i35_i18n_formas.dart';
import 'package:ngdart/src/core/linker/views/component_view.dart' as import0;
import 'i35_i18n_formas.dart' as import1;
import 'package:intl/intl.dart' as import2;
import 'package:ngdart/src/core/linker/style_encapsulation.dart' as import3;
import 'package:ngdart/src/core/linker/views/view.dart' as import4;
import 'package:ngdart/src/meta/change_detection_constants.dart' as import5;
import 'package:ngdart/src/utilities.dart' as import6;
import 'dart:html' as import7;
import 'package:ngdart/src/runtime/dom_helpers.dart' as import8;
import 'package:ngdart/angular.dart';
import 'package:ngdart/src/core/linker/views/host_view.dart' as import10;

final List<Object> styles$I35I18nFormas = const [];

class ViewI35I18nFormas0 extends import0.ComponentView<import1.I35I18nFormas> {
  static final String _message_0 = import2.Intl.message('Olá mundo', desc: 'd1', meaning: 'm', skip: true);
  static final String _message_1 = import2.Intl.message('Salvar', desc: 'botão', locale: 'pt');
  static import3.ComponentStyles? _componentStyles;
  ViewI35I18nFormas0(import4.View parentView, int parentIndex) : super(parentView, parentIndex, import5.ChangeDetectionCheckedState.checkAlways) {
    this.initComponentStyles();
    this.rootElement = import6.unsafeCast(import7.document.createElement('i35-i18n-formas'));
  }
  static String? get _debugComponentUrl {
    return (import6.isDevMode ? 'asset:corpus_ngdart/lib/src/i35_i18n_formas.dart' : null);
  }

  @override
  void build() {
    final parentRenderNode = this.initViewRoot();
    final doc = import7.document;
    final _el_0 = import8.appendElement<import7.HtmlElement>(doc, parentRenderNode, 'p');
    final _text_1 = import8.appendText(_el_0, _message_0);
    final _el_2 = import8.appendElement<import7.ButtonElement>(doc, parentRenderNode, 'button');
    import8.setAttribute(_el_2, 'title', _message_1);
    final _text_3 = import8.appendText(_el_2, 'x');
    final _el_4 = import8.appendSpan(doc, parentRenderNode);
    final _text_5 = import8.appendText(_el_4, _message_0);
  }

  static void _debugClearComponentStyles() {
    _componentStyles = null;
  }

  void initComponentStyles() {
    var styles = _componentStyles;
    if ((styles == null)) {
      _componentStyles = (styles = import3.ComponentStyles.unscoped(styles$I35I18nFormas, _debugComponentUrl));
      if (import6.isDevMode) {
        import3.ComponentStyles.debugOnClear(_debugClearComponentStyles);
      }
    }
    this.componentStyles = styles;
  }
}

const _I35I18nFormasNgFactory = ComponentFactory<import1.I35I18nFormas>('i35-i18n-formas', viewFactory_I35I18nFormasHost0);
ComponentFactory<import1.I35I18nFormas> get I35I18nFormasNgFactory {
  return _I35I18nFormasNgFactory;
}

ComponentFactory<import1.I35I18nFormas> createI35I18nFormasFactory() {
  return ComponentFactory('i35-i18n-formas', viewFactory_I35I18nFormasHost0);
}

final List<Object> styles$I35I18nFormasHost = const [];

class _ViewI35I18nFormasHost0 extends import10.HostView<import1.I35I18nFormas> {
  @override
  void build() {
    this.componentView = ViewI35I18nFormas0(this, 0);
    final _el_0 = this.componentView.rootElement;
    this.component = import1.I35I18nFormas();
    this.initRootNode(_el_0);
  }
}

import10.HostView<import1.I35I18nFormas> viewFactory_I35I18nFormasHost0() {
  return _ViewI35I18nFormasHost0();
}
