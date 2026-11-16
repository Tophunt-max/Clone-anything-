package com.tophunt.forge.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.ColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Shapes
import androidx.compose.material3.Typography
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.TextUnit
import androidx.compose.ui.unit.em

/** Material 3 colours from the shared tokens (packages/design-tokens/tokens.json). */
fun forgeColorScheme(dark: Boolean): ColorScheme =
    if (dark) {
        val t = ForgeTokens.Dark
        darkColorScheme(
            primary = t.accentStrong,
            onPrimary = t.onAccent,
            secondary = t.accentViolet,
            onSecondary = t.bgCanvas,
            background = t.bgCanvas,
            onBackground = t.textPrimary,
            surface = t.bgSurface,
            onSurface = t.textPrimary,
            surfaceVariant = t.bgRaised,
            onSurfaceVariant = t.textSecondary,
            surfaceContainer = t.bgRaised,
            outline = t.borderStrong,
            outlineVariant = t.borderSubtle,
            error = t.danger,
            onError = t.bgCanvas,
        )
    } else {
        val t = ForgeTokens.Light
        lightColorScheme(
            primary = t.accentStrong,
            onPrimary = t.onAccent,
            secondary = t.accentViolet,
            onSecondary = t.onAccent,
            background = t.bgCanvas,
            onBackground = t.textPrimary,
            surface = t.bgSurface,
            onSurface = t.textPrimary,
            surfaceVariant = t.bgRaised,
            onSurfaceVariant = t.textSecondary,
            surfaceContainer = t.bgRaised,
            outline = t.borderStrong,
            outlineVariant = t.borderSubtle,
            error = t.danger,
            onError = t.onAccent,
        )
    }

private fun style(size: TextUnit, weight: FontWeight = FontWeight.Normal, family: FontFamily = FontFamily.Default) =
    TextStyle(fontSize = size, fontWeight = weight, fontFamily = family, lineHeight = ForgeTokens.LINE_HEIGHT.em)

/** Type scale from the tokens. Inter/JetBrains Mono font files are bundled in Phase 4. */
val forgeTypography =
    Typography(
        headlineMedium = style(ForgeTokens.FontSize.title, FontWeight.SemiBold),
        titleLarge = style(ForgeTokens.FontSize.section, FontWeight.SemiBold),
        titleMedium = style(ForgeTokens.FontSize.lead, FontWeight.Medium),
        bodyLarge = style(ForgeTokens.FontSize.body),
        bodyMedium = style(ForgeTokens.FontSize.small),
        bodySmall = style(ForgeTokens.FontSize.caption),
        labelSmall = style(ForgeTokens.FontSize.tiny, FontWeight.Medium),
    )

val forgeShapes =
    Shapes(
        extraSmall = RoundedCornerShape(ForgeTokens.Radius.sm),
        small = RoundedCornerShape(ForgeTokens.Radius.md),
        medium = RoundedCornerShape(ForgeTokens.Radius.lg),
        large = RoundedCornerShape(ForgeTokens.Radius.xl),
    )

@Composable
fun ForgeTheme(dark: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    MaterialTheme(colorScheme = forgeColorScheme(dark), typography = forgeTypography, shapes = forgeShapes, content = content)
}
