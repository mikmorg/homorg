import 'package:flutter_test/flutter_test.dart';
import 'package:homorg/main.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  testWidgets('App builds without error', (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    await tester.pumpWidget(const HomorgApp());
    // The app now shows a loading spinner briefly, then either login or home.
    // Just verify it builds without throwing.
    await tester.pump();
    expect(find.byType(HomorgApp), findsOneWidget);
  });
}
