import 'package:flutter_test/flutter_test.dart';
import 'package:cowallet/main.dart';

void main() {
  testWidgets('App renders', (WidgetTester tester) async {
    await tester.pumpWidget(const CowalletApp());
    await tester.pumpAndSettle();
    expect(find.text('cowallet'), findsWidgets);
  });
}
