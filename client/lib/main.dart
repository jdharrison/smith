import 'package:flutter/material.dart';
import 'package:google_fonts/google_fonts.dart';
import 'package:smith_client/chat/chat_controller.dart';
import 'package:smith_client/home/home_screen.dart';
import 'package:smith_client/listen/voice_turn.dart';

void main() {
  WidgetsFlutterBinding.ensureInitialized();
  runApp(const SmithClientApp());
}

class SmithClientApp extends StatefulWidget {
  const SmithClientApp({super.key});

  @override
  State<SmithClientApp> createState() => _SmithClientAppState();
}

class _SmithClientAppState extends State<SmithClientApp> {
  late final ChatController _chat = ChatController();
  late final VoiceTurnController _voice = VoiceTurnController(chat: _chat);

  @override
  void dispose() {
    _voice.dispose();
    _chat.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final base = ThemeData(
      useMaterial3: true,
      brightness: Brightness.light,
      scaffoldBackgroundColor: const Color(0xFFF0F3F6),
      colorScheme: const ColorScheme.light(
        primary: Color(0xFF12353F),
        secondary: Color(0xFF1A6B78),
        surface: Color(0xFFF0F3F6),
        onPrimary: Color(0xFFF3F7F8),
        onSurface: Color(0xFF15262D),
      ),
    );

    return MaterialApp(
      title: 'smith client',
      debugShowCheckedModeBanner: false,
      theme: base.copyWith(
        textTheme: GoogleFonts.dmSansTextTheme(base.textTheme),
      ),
      home: HomeScreen(chat: _chat, voice: _voice),
    );
  }
}
