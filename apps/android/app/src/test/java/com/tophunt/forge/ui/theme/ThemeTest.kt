package com.tophunt.forge.ui.theme

import org.junit.Assert.assertEquals
import org.junit.Test

class ThemeTest {
    @Test
    fun darkSchemeUsesTheSharedTokens() {
        val s = forgeColorScheme(dark = true)
        assertEquals(ForgeTokens.Dark.bgCanvas, s.background)
        assertEquals(ForgeTokens.Dark.bgSurface, s.surface)
        assertEquals(ForgeTokens.Dark.textPrimary, s.onSurface)
        assertEquals(ForgeTokens.Dark.accentStrong, s.primary)
    }

    @Test
    fun lightSchemeUsesTheSharedTokens() {
        val s = forgeColorScheme(dark = false)
        assertEquals(ForgeTokens.Light.bgCanvas, s.background)
        assertEquals(ForgeTokens.Light.textPrimary, s.onBackground)
        assertEquals(ForgeTokens.Light.danger, s.error)
    }

    @Test
    fun generatedTokensMatchTheJsonSource() {
        // Same values as packages/design-tokens/tokens.json; the token build's --check keeps the
        // generated file in sync, this guards the Kotlin mapping itself.
        assertEquals(0xFF0B1020.toInt(), ForgeTokens.Dark.bgCanvas.value.shr(32).toInt())
        assertEquals(1280, ForgeTokens.BREAKPOINT_LARGE_DP)
    }
}
