package io.github.space2233.pixnya

import android.app.Activity
import android.content.Intent
import android.net.ConnectivityManager
import android.net.Uri
import android.os.Build
import android.content.pm.PackageInfo
import android.content.pm.PackageManager
import android.provider.Settings
import androidx.core.content.FileProvider
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.File
import java.io.FileInputStream
import java.security.MessageDigest
import java.util.zip.ZipFile

@InvokeArg
class InstallUpdateArgs {
  var apkPath: String? = null
  var expectedVersionCode: Long = 0
  var expectedSize: Long = 0
  var expectedSha256: String? = null
  var expectedCertificateSha256: String? = null
  var expectedAbi: String? = null
}

@TauriPlugin
class UpdateInstallerPlugin(private val activity: Activity) : Plugin(activity) {
  companion object {
    private const val UPDATE_DIRECTORY = "updates"
    private val APK_NAME = Regex("pixnya-[A-Za-z0-9._-]{1,96}\\.apk")
  }

  @Command
  fun getInstallStatus(invoke: Invoke) {
    invoke.resolve(installStatus())
  }

  @Command
  fun requestInstallPermission(invoke: Invoke) {
    if (canRequestPackageInstalls()) {
      invoke.resolve(installStatus())
      return
    }
    activity.runOnUiThread {
      try {
        val intent = Intent(
          Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES,
          Uri.parse("package:${activity.packageName}"),
        )
        activity.startActivity(intent)
        val result = installStatus()
        result.put("awaitingSystemAction", true)
        invoke.resolve(result)
      } catch (error: Exception) {
        invoke.reject("Unable to open install permission settings", "install_permission_unavailable", error)
      }
    }
  }

  @Command
  fun installApk(invoke: Invoke) {
    val args = invoke.parseArgs(InstallUpdateArgs::class.java)
    val value = args.apkPath
    if (value.isNullOrBlank()) {
      invoke.reject("Missing update package", "invalid_update_package")
      return
    }
    if (!canRequestPackageInstalls()) {
      invoke.reject("Install permission is required", "install_permission_required")
      return
    }

    activity.runOnUiThread {
      try {
        val allowedRoot = File(activity.cacheDir, UPDATE_DIRECTORY).canonicalFile
        val apk = File(value).canonicalFile
        check(apk.parentFile == allowedRoot) { "Update package is outside the private update directory" }
        check(APK_NAME.matches(apk.name)) { "Invalid update package name" }
        check(apk.isFile && apk.length() > 0L) { "Update package is unavailable" }
        verifyUpdatePackage(apk, args)

        val uri = FileProvider.getUriForFile(
          activity,
          "${activity.packageName}.fileprovider",
          apk,
        )
        val intent = Intent(Intent.ACTION_VIEW).apply {
          setDataAndType(uri, "application/vnd.android.package-archive")
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
          addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        }
        check(intent.resolveActivity(activity.packageManager) != null) {
          "No Android package installer is available"
        }
        activity.startActivity(intent)
        val result = installStatus()
        result.put("awaitingSystemAction", true)
        invoke.resolve(result)
      } catch (error: Exception) {
        invoke.reject("Unable to open the Android package installer", "package_installer_unavailable", error)
      }
    }
  }

  private fun canRequestPackageInstalls(): Boolean =
    Build.VERSION.SDK_INT < Build.VERSION_CODES.O || activity.packageManager.canRequestPackageInstalls()

  private fun verifyUpdatePackage(apk: File, args: InstallUpdateArgs) {
    val expectedSha256 = normalizeDigest(args.expectedSha256)
    val expectedCertificate = normalizeDigest(args.expectedCertificateSha256)
    val expectedAbi = args.expectedAbi
    check(args.expectedVersionCode > 0L) { "Invalid expected update version" }
    check(args.expectedSize > 0L && apk.length() == args.expectedSize) {
      "Update package size does not match the signed manifest"
    }
    check(expectedAbi in setOf("arm64-v8a", "armeabi-v7a")) {
      "Unsupported update architecture"
    }
    check(sha256(apk) == expectedSha256) {
      "Update package digest does not match the signed manifest"
    }
    verifyApkAbi(apk, expectedAbi!!)

    val archive = archivePackageInfo(apk)
      ?: error("Android could not inspect the update package")
    val installed = installedPackageInfo()
    check(archive.packageName == activity.packageName) {
      "Update package belongs to another application"
    }
    check(archive.longVersionCode == args.expectedVersionCode) {
      "Update package version does not match the signed manifest"
    }
    check(archive.longVersionCode > installed.longVersionCode) {
      "Update package is not newer than the installed application"
    }
    val archiveCertificate = signingCertificateSha256(archive)
    val installedCertificate = signingCertificateSha256(installed)
    check(archiveCertificate == expectedCertificate) {
      "Update package certificate does not match the signed manifest"
    }
    check(archiveCertificate == installedCertificate) {
      "Update package certificate does not match the installed application"
    }
  }

  @Suppress("DEPRECATION")
  private fun archivePackageInfo(apk: File): PackageInfo? =
    activity.packageManager.getPackageArchiveInfo(
      apk.absolutePath,
      PackageManager.GET_SIGNING_CERTIFICATES,
    )

  @Suppress("DEPRECATION")
  private fun installedPackageInfo(): PackageInfo =
    activity.packageManager.getPackageInfo(
      activity.packageName,
      PackageManager.GET_SIGNING_CERTIFICATES,
    )

  private fun signingCertificateSha256(info: PackageInfo): String {
    val signers = info.signingInfo?.apkContentsSigners
      ?: error("Package signing certificate is unavailable")
    check(signers.size == 1) { "Multiple package signers are not supported" }
    return digest(signers[0].toByteArray())
  }

  private fun verifyApkAbi(apk: File, expectedAbi: String) {
    val expectedPrefix = "lib/$expectedAbi/"
    var foundExpected = false
    ZipFile(apk).use { archive ->
      val entries = archive.entries()
      while (entries.hasMoreElements()) {
        val name = entries.nextElement().name
        if (name.startsWith(expectedPrefix) && name.endsWith(".so")) {
          foundExpected = true
        }
        check(!name.startsWith("lib/") || !name.endsWith(".so") || name.startsWith(expectedPrefix)) {
          "Update package contains a different native architecture"
        }
      }
    }
    check(foundExpected) { "Update package does not contain the expected native architecture" }
  }

  private fun sha256(file: File): String {
    val messageDigest = MessageDigest.getInstance("SHA-256")
    FileInputStream(file).use { input ->
      val buffer = ByteArray(64 * 1024)
      while (true) {
        val read = input.read(buffer)
        if (read < 0) break
        messageDigest.update(buffer, 0, read)
      }
    }
    return digest(messageDigest.digest())
  }

  private fun digest(bytes: ByteArray): String =
    bytes.joinToString(separator = "") { byte -> "%02x".format(byte.toInt() and 0xff) }

  private fun normalizeDigest(value: String?): String {
    val normalized = value.orEmpty().replace(":", "").lowercase()
    check(normalized.matches(Regex("[0-9a-f]{64}"))) { "Invalid expected package digest" }
    return normalized
  }

  private fun installStatus(): JSObject {
    val result = JSObject()
    result.put("canRequestPackageInstalls", canRequestPackageInstalls())
    result.put("requiresUserConfirmation", true)
    result.put("awaitingSystemAction", false)
    result.put("sdkInt", Build.VERSION.SDK_INT)
    val connectivity = activity.getSystemService(ConnectivityManager::class.java)
    result.put("activeNetworkMetered", connectivity?.isActiveNetworkMetered ?: true)
    return result
  }
}
