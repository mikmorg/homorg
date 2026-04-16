import 'package:flutter/material.dart';

import 'services/auth_service.dart';
import 'screens/login_screen.dart';
import 'screens/home_screen.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const HomorgApp());
}

class HomorgApp extends StatefulWidget {
  const HomorgApp({super.key});

  @override
  State<HomorgApp> createState() => _HomorgAppState();
}

class _HomorgAppState extends State<HomorgApp> {
  final _auth = AuthService();
  bool _ready = false;

  @override
  void initState() {
    super.initState();
    _init();
  }

  Future<void> _init() async {
    await _auth.loadStored();
    if (mounted) setState(() => _ready = true);
  }

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Homorg',
      debugShowCheckedModeBanner: false,
      theme: ThemeData(
        colorScheme: ColorScheme.fromSeed(
          seedColor: const Color(0xFF4F46E5),
          brightness: Brightness.dark,
        ),
        useMaterial3: true,
      ),
      home: _ready
          ? (_auth.isLoggedIn
              ? HomeScreen(auth: _auth)
              : LoginScreen(auth: _auth))
          : const Scaffold(
              body: Center(child: CircularProgressIndicator()),
            ),
    );
  }
}
