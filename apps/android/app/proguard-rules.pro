# kotlinx.serialization: keep generated serializers of the wire types.
-keepattributes *Annotation*, InnerClasses
-keepclassmembers class com.tophunt.forge.data.** {
    *** Companion;
    kotlinx.serialization.KSerializer serializer(...);
}
-keep,includedescriptorclasses class com.tophunt.forge.data.**$$serializer { *; }
# Retrofit interfaces are used through reflection.
-keep,allowobfuscation interface com.tophunt.forge.data.ForgeService
-keepattributes Signature, Exceptions
-dontwarn org.slf4j.**
