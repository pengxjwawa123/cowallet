import 'package:flutter/material.dart';
import '../l10n/strings.dart';

class AppState extends ChangeNotifier {
  Lang _lang = Lang.zh;
  String _userName = '';
  String _persona = 'daily';
  bool _onboardingComplete = false;
  String _walletAddress = '';
  bool _walletLoading = false;

  Lang get lang => _lang;
  String get userName => _userName;
  String get persona => _persona;
  bool get onboardingComplete => _onboardingComplete;
  String get walletAddress => _walletAddress;
  bool get walletLoading => _walletLoading;
  bool get hasWallet => _walletAddress.isNotEmpty;

  void setLang(Lang l) {
    _lang = l;
    S.setLang(l);
    notifyListeners();
  }

  void setUserName(String name) {
    _userName = name;
    notifyListeners();
  }

  void setPersona(String p) {
    _persona = p;
    notifyListeners();
  }

  void completeOnboarding() {
    _onboardingComplete = true;
    notifyListeners();
  }

  void resetOnboarding() {
    _onboardingComplete = false;
    notifyListeners();
  }

  void setWalletAddress(String addr) {
    _walletAddress = addr;
    notifyListeners();
  }

  void setWalletLoading(bool v) {
    _walletLoading = v;
    notifyListeners();
  }
}
