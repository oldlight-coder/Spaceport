package com.spaceport.client

import android.app.Activity
import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.WebView
import androidx.appcompat.app.AppCompatActivity
import app.tauri.annotation.Command
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Spaceport's Android-only native pieces, called from Rust (src-tauri/src/mobile.rs).
 *
 * Secrets: an AES-256 key is created in the Android Keystore (it never leaves it); each secret
 * is encrypted with it (AES-GCM) and only the ciphertext is kept, in the app's private storage.
 *
 * Player: Media3 in SpaceportPlayer, driven by src-tauri/src/player/exo.rs.
 */
@TauriPlugin
class SpaceportPlugin(private val activity: Activity) : Plugin(activity) {
  private val player = SpaceportPlayer(activity)

  override fun load(webView: WebView) {
    player.attach(webView)
  }

  override fun onPause(activity: AppCompatActivity) {
    player.pause()
  }

  override fun onDestroy(activity: AppCompatActivity) {
    player.close()
  }

  /** Runs `work` on the UI thread, where Media3 and the views must be used, then answers. */
  private fun onUiThread(invoke: Invoke, work: () -> JSObject?) {
    activity.runOnUiThread {
      try {
        val result = work()
        if (result == null) invoke.resolve() else invoke.resolve(result)
      } catch (e: Exception) {
        invoke.reject("player: ${e.message}")
      }
    }
  }

  @Command
  fun playerOpen(invoke: Invoke) = onUiThread(invoke) {
    player.open()
    null
  }

  @Command
  fun playerOp(invoke: Invoke) = onUiThread(invoke) {
    player.op(invoke.getArgs())
    null
  }

  @Command
  fun playerPoll(invoke: Invoke) = onUiThread(invoke) { player.poll() }

  @Command
  fun playerClose(invoke: Invoke) = onUiThread(invoke) {
    player.close()
    null
  }

  /** Whether the active network is Wi-Fi or Ethernet, for "torrents only on Wi-Fi". */
  @Command
  fun networkStatus(invoke: Invoke) {
    val connectivity = activity.getSystemService(ConnectivityManager::class.java)
    val capabilities = connectivity?.getNetworkCapabilities(connectivity.activeNetwork)
    val result = JSObject()
    result.put(
      "wifi",
      capabilities != null &&
        (capabilities.hasTransport(NetworkCapabilities.TRANSPORT_WIFI) ||
          capabilities.hasTransport(NetworkCapabilities.TRANSPORT_ETHERNET)),
    )
    invoke.resolve(result)
  }

  private val prefs by lazy {
    activity.getSharedPreferences("spaceport_secrets", Context.MODE_PRIVATE)
  }

  private fun key(): SecretKey {
    val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
    (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
    val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
    generator.init(
      KeyGenParameterSpec.Builder(
        KEY_ALIAS,
        KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
      )
        .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
        .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
        .setKeySize(256)
        .build()
    )
    return generator.generateKey()
  }

  private fun encrypt(plain: String): String {
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(Cipher.ENCRYPT_MODE, key())
    val sealed = cipher.iv + cipher.doFinal(plain.toByteArray(Charsets.UTF_8))
    return Base64.encodeToString(sealed, Base64.NO_WRAP)
  }

  private fun decrypt(stored: String): String {
    val sealed = Base64.decode(stored, Base64.NO_WRAP)
    val cipher = Cipher.getInstance(TRANSFORMATION)
    cipher.init(
      Cipher.DECRYPT_MODE,
      key(),
      GCMParameterSpec(128, sealed.copyOfRange(0, IV_LENGTH)),
    )
    return String(cipher.doFinal(sealed.copyOfRange(IV_LENGTH, sealed.size)), Charsets.UTF_8)
  }

  @Command
  fun secretGet(invoke: Invoke) {
    try {
      val name = invoke.getArgs().getString("name")
      val result = JSObject()
      prefs.getString(name, null)?.let { result.put("value", decrypt(it)) }
      invoke.resolve(result)
    } catch (e: Exception) {
      invoke.reject("could not read the secret: ${e.message}")
    }
  }

  @Command
  fun secretSet(invoke: Invoke) {
    try {
      val args = invoke.getArgs()
      prefs.edit().putString(args.getString("name"), encrypt(args.getString("value"))).apply()
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("could not store the secret: ${e.message}")
    }
  }

  @Command
  fun secretDelete(invoke: Invoke) {
    try {
      prefs.edit().remove(invoke.getArgs().getString("name")).apply()
      invoke.resolve()
    } catch (e: Exception) {
      invoke.reject("could not remove the secret: ${e.message}")
    }
  }

  private companion object {
    const val KEYSTORE = "AndroidKeyStore"
    const val KEY_ALIAS = "spaceport_secrets"
    const val TRANSFORMATION = "AES/GCM/NoPadding"
    const val IV_LENGTH = 12
  }
}
