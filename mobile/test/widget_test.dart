import 'package:flutter_test/flutter_test.dart';
import 'package:homorg_camera/main.dart';

void main() {
  testWidgets('App builds without error', (WidgetTester tester) async {
    await tester.pumpWidget(const HomorgCameraApp());
    expect(find.text('Homorg Camera'), findsOneWidget);
  });
}
