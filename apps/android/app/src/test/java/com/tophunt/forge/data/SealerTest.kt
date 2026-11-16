package com.tophunt.forge.data

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Test
import javax.crypto.KeyGenerator

/** Token sealing (the Keystore key is replaced by a software key on the JVM). */
class SealerTest {
    private val key = KeyGenerator.getInstance("AES").apply { init(256) }.generateKey()
    private val sealer = Sealer { key }

    @Test
    fun roundTripsWithAFreshIvEachTime() {
        val plain = """{"access":"fa1.x","refresh":"fr1.y"}""".encodeToByteArray()
        val a = sealer.seal(plain)
        val b = sealer.seal(plain)
        assertNotEquals(a, b)
        assertArrayEquals(plain, sealer.open(a))
        assertNotEquals(true, a.contains("fr1.y"))
    }

    @Test
    fun tamperedForeignOrGarbageDataOpensToNothing() {
        val sealed = sealer.seal("secret".encodeToByteArray())
        val bytes = java.util.Base64.getDecoder().decode(sealed)
        bytes[bytes.size - 1] = (bytes.last().toInt() xor 1).toByte()
        assertNull(sealer.open(java.util.Base64.getEncoder().encodeToString(bytes)))
        val other = Sealer { KeyGenerator.getInstance("AES").apply { init(256) }.generateKey() }
        assertNull(other.open(sealed))
        assertNull(sealer.open("not base64 !!"))
        assertNull(sealer.open(""))
    }
}
