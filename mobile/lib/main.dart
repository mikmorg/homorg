import 'package:flutter/material.dart';

import 'screens/connect_screen.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const HomorgCameraApp());
}

class HomorgCameraApp extends StatelessWidget {
  const HomorgCameraApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Homorg Camera',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF4F46E5), // indigo, matches web UI
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
      home: const ConnectScreen(),
    );
  }
}
