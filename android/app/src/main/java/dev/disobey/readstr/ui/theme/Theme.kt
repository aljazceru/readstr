package dev.disobey.readstr.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.googlefonts.Font
import androidx.compose.ui.text.googlefonts.GoogleFont
import androidx.compose.material3.Typography
import androidx.compose.ui.unit.sp
import dev.disobey.readstr.R

// ── Google Fonts ─────────────────────────────────────────────────────────────

val GoogleFontsProvider = GoogleFont.Provider(
    providerAuthority = "com.google.android.gms.fonts",
    providerPackage = "com.google.android.gms",
    certificates = R.array.com_google_android_gms_fonts_certs
)

val SyneFontFamily = FontFamily(
    Font(
        googleFont = GoogleFont("Syne"),
        fontProvider = GoogleFontsProvider,
        weight = FontWeight.Bold
    )
)

val JetBrainsMonoFontFamily = FontFamily(
    Font(
        googleFont = GoogleFont("JetBrains Mono"),
        fontProvider = GoogleFontsProvider,
        weight = FontWeight.Normal
    ),
    Font(
        googleFont = GoogleFont("JetBrains Mono"),
        fontProvider = GoogleFontsProvider,
        weight = FontWeight.Bold
    )
)

// ── Palette ──────────────────────────────────────────────────────────────────

val AccentOrange = Color(0xFFFF6B2B)
val AccentOrangeDim = Color(0xFF3D1A08)

private val DarkBackground    = Color(0xFF0E0C12)
private val DarkSurface       = Color(0xFF161320)
private val DarkSurfaceVar    = Color(0xFF1F1B2C)
private val DarkOnBg          = Color(0xFFEAE6F4)
private val DarkOnSurface     = Color(0xFFEAE6F4)
private val DarkOnSurfaceVar  = Color(0xFF9490A8)
private val DarkOutline       = Color(0xFF38344A)

private val DarkScheme = darkColorScheme(
    primary              = AccentOrange,
    onPrimary            = Color(0xFF1A0A00),
    primaryContainer     = AccentOrangeDim,
    onPrimaryContainer   = Color(0xFFFFD0B8),
    secondary            = Color(0xFF9490A8),
    onSecondary          = DarkBackground,
    background           = DarkBackground,
    onBackground         = DarkOnBg,
    surface              = DarkSurface,
    onSurface            = DarkOnSurface,
    surfaceVariant       = DarkSurfaceVar,
    onSurfaceVariant     = DarkOnSurfaceVar,
    error                = Color(0xFFFF5555),
    onError              = Color(0xFF1A0000),
    outline              = DarkOutline,
    outlineVariant       = Color(0xFF2A2638),
)

private val LightBackground   = Color(0xFFF6F3FF)
private val LightSurface      = Color(0xFFFFFFFF)
private val LightSurfaceVar   = Color(0xFFEDE8F6)
private val LightOnBg         = Color(0xFF1A1825)
private val LightAccent       = Color(0xFFE5520A)

private val LightScheme = lightColorScheme(
    primary              = LightAccent,
    onPrimary            = Color(0xFFFFFFFF),
    primaryContainer     = Color(0xFFFFDDD0),
    onPrimaryContainer   = Color(0xFF3A1200),
    secondary            = Color(0xFF6B5E6D),
    onSecondary          = Color(0xFFFFFFFF),
    background           = LightBackground,
    onBackground         = LightOnBg,
    surface              = LightSurface,
    onSurface            = LightOnBg,
    surfaceVariant       = LightSurfaceVar,
    onSurfaceVariant     = Color(0xFF5E5870),
    error                = Color(0xFFCC2222),
    onError              = Color(0xFFFFFFFF),
    outline              = Color(0xFFCCC8D8),
    outlineVariant       = Color(0xFFE0DBF0),
)

// ── Typography ───────────────────────────────────────────────────────────────

private val AppTypography = Typography(
    displayLarge  = Typography().displayLarge.copy(fontFamily = SyneFontFamily),
    displayMedium = Typography().displayMedium.copy(fontFamily = SyneFontFamily),
    displaySmall  = Typography().displaySmall.copy(fontFamily = SyneFontFamily),
    headlineLarge = Typography().headlineLarge.copy(fontFamily = SyneFontFamily, fontWeight = FontWeight.Bold),
    headlineMedium= Typography().headlineMedium.copy(fontFamily = SyneFontFamily, fontWeight = FontWeight.Bold),
    headlineSmall = Typography().headlineSmall.copy(fontFamily = SyneFontFamily),
    titleLarge    = Typography().titleLarge.copy(letterSpacing = 0.02.sp),
    labelSmall    = Typography().labelSmall.copy(letterSpacing = 0.08.sp),
)

// ── Theme ────────────────────────────────────────────────────────────────────

@Composable
fun AppTheme(darkTheme: Boolean = true, content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = if (darkTheme) DarkScheme else LightScheme,
        typography = AppTypography,
        content = content
    )
}
