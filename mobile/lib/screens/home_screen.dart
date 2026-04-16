import 'package:flutter/material.dart';

import '../services/auth_service.dart';
import '../services/homorg_api.dart';
import 'browse_screen.dart';
import 'login_screen.dart';
import 'scan_screen.dart';
import 'stocker_landing_screen.dart';

class HomeScreen extends StatefulWidget {
  final AuthService auth;

  const HomeScreen({super.key, required this.auth});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  int _tabIndex = 0;
  late final HomorgApi _api;

  @override
  void initState() {
    super.initState();
    _api = HomorgApi(widget.auth);
  }

  Future<void> _logout() async {
    await widget.auth.logout();
    if (!mounted) return;
    Navigator.of(context).pushReplacement(
      MaterialPageRoute(
        builder: (_) => LoginScreen(auth: widget.auth),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Homorg'),
        actions: [
          if (widget.auth.user != null)
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 8),
              child: Center(
                child: Text(
                  widget.auth.user!.displayName ?? widget.auth.user!.username,
                  style: Theme.of(context).textTheme.bodySmall,
                ),
              ),
            ),
          PopupMenuButton<String>(
            onSelected: (value) {
              if (value == 'logout') _logout();
            },
            itemBuilder: (_) => [
              const PopupMenuItem(
                value: 'logout',
                child: Row(
                  children: [
                    Icon(Icons.logout, size: 18),
                    SizedBox(width: 8),
                    Text('Sign out'),
                  ],
                ),
              ),
            ],
          ),
        ],
      ),
      body: IndexedStack(
        index: _tabIndex,
        children: [
          ScanScreen(api: _api),
          BrowseScreen(api: _api),
          StockerLandingScreen(api: _api),
        ],
      ),
      bottomNavigationBar: NavigationBar(
        selectedIndex: _tabIndex,
        onDestinationSelected: (i) => setState(() => _tabIndex = i),
        destinations: const [
          NavigationDestination(
            icon: Icon(Icons.qr_code_scanner_outlined),
            selectedIcon: Icon(Icons.qr_code_scanner),
            label: 'Scan',
          ),
          NavigationDestination(
            icon: Icon(Icons.folder_outlined),
            selectedIcon: Icon(Icons.folder),
            label: 'Browse',
          ),
          NavigationDestination(
            icon: Icon(Icons.inventory_outlined),
            selectedIcon: Icon(Icons.inventory),
            label: 'Stocker',
          ),
        ],
      ),
    );
  }
}
