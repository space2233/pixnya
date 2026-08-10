package io.github.space2233.pixnya

import android.app.Activity
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.OpenableColumns
import androidx.activity.result.ActivityResult
import app.tauri.annotation.ActivityCallback
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin
import java.io.ByteArrayOutputStream
import java.io.File
import java.io.FileInputStream
import java.nio.file.Files
import org.json.JSONObject

@InvokeArg
class ExportDirectoryArgs {
  var sourceDirectory: String? = null
  var directoryName: String? = null
  var expectedFileCount: Int = 0
  var expectedSizeBytes: Long = 0
}

private data class DocumentChild(
  val uri: Uri,
  val mimeType: String,
)

@TauriPlugin
class ExportDirectoryPlugin(private val activity: Activity) : Plugin(activity) {
  companion object {
    private const val PREFERENCES = "pixiv-client-export-destination"
    private const val URI_KEY = "tree-uri"
    private const val LABEL_KEY = "tree-label"
    private const val EXPORT_STAGING_DIRECTORY = "export-staging-v1"
    private const val EXPORT_MARKER_FILE = "pixiv-client-entry.json"
    private const val MAX_EXPORT_FILES = 4096
    private const val MAX_MARKER_BYTES = 1024 * 1024
    private val DIRECTORY_NAME = Regex("(?:artwork|novel|ugoira)-[1-9][0-9]{0,19}")
    private val FILE_NAME = Regex("[A-Za-z0-9][A-Za-z0-9._-]{0,127}")
  }

  @Command
  fun getDirectoryStatus(invoke: Invoke) {
    try {
      invoke.resolve(statusObject())
    } catch (error: Exception) {
      invoke.reject("Unable to read the export directory", "export_destination_unavailable", error)
    }
  }

  @Command
  fun selectDirectory(invoke: Invoke) {
    activity.runOnUiThread {
      try {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
          addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
          addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
          addFlags(Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
          addFlags(Intent.FLAG_GRANT_PREFIX_URI_PERMISSION)
        }
        startActivityForResult(invoke, intent, "onDirectorySelected")
      } catch (error: Exception) {
        invoke.reject("Unable to open the directory picker", "export_destination_unavailable", error)
      }
    }
  }

  @ActivityCallback
  private fun onDirectorySelected(invoke: Invoke, result: ActivityResult) {
    if (result.resultCode != Activity.RESULT_OK) {
      val status = statusObject()
      status.put("cancelled", true)
      invoke.resolve(status)
      return
    }

    val data = result.data
    val uri = data?.data
    if (uri == null || !DocumentsContract.isTreeUri(uri)) {
      invoke.reject("The selected location is not a document tree", "invalid_export_destination")
      return
    }

    try {
      val requestedFlags = Intent.FLAG_GRANT_READ_URI_PERMISSION or
        Intent.FLAG_GRANT_WRITE_URI_PERMISSION
      val grantedFlags = (data.flags and requestedFlags)
      if (grantedFlags != requestedFlags) {
        invoke.reject("The selected directory is not writable", "export_permission_unavailable")
        return
      }
      activity.contentResolver.takePersistableUriPermission(uri, grantedFlags)
      if (!hasPersistedAccess(uri)) {
        invoke.reject("The directory permission could not be retained", "export_permission_unavailable")
        return
      }

      val preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      val previous = preferences.getString(URI_KEY, null)?.let(Uri::parse)
      val label = displayName(uri)
      val saved = preferences.edit()
        .putString(URI_KEY, uri.toString())
        .putString(LABEL_KEY, label)
        .commit()
      check(saved) { "Export destination did not commit" }
      if (previous != null && previous != uri) {
        try {
          activity.contentResolver.releasePersistableUriPermission(previous, requestedFlags)
        } catch (_: Exception) {
          // The old provider may already have revoked the permission.
        }
      }
      val status = statusObject()
      status.put("cancelled", false)
      invoke.resolve(status)
    } catch (error: Exception) {
      invoke.reject("Unable to retain the directory permission", "export_permission_unavailable", error)
    }
  }

  @Command
  fun clearDirectory(invoke: Invoke) {
    try {
      val preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      preferences.getString(URI_KEY, null)?.let { stored ->
        try {
          activity.contentResolver.releasePersistableUriPermission(
            Uri.parse(stored),
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
          )
        } catch (_: Exception) {
          // Clearing local state must still succeed if the provider revoked access first.
        }
      }
      val cleared = preferences.edit().clear().commit()
      check(cleared) { "Export destination did not clear" }
      invoke.resolve()
    } catch (error: Exception) {
      invoke.reject("Unable to clear the export directory", "export_destination_unavailable", error)
    }
  }

  @Command
  fun exportDirectory(invoke: Invoke) {
    val args = invoke.parseArgs(ExportDirectoryArgs::class.java)
    val directoryName = args.directoryName
    val sourceValue = args.sourceDirectory
    if (
      directoryName == null ||
      !DIRECTORY_NAME.matches(directoryName) ||
      sourceValue.isNullOrBlank() ||
      args.expectedFileCount !in 1..MAX_EXPORT_FILES ||
      args.expectedSizeBytes < 0
    ) {
      invoke.reject("Invalid export request", "invalid_export")
      return
    }

    try {
      val treeUri = currentTreeUri()
        ?: return invoke.reject("No export directory is configured", "export_destination_unavailable")
      if (!hasPersistedAccess(treeUri)) {
        clearStoredDestination()
        invoke.reject("The export directory permission has expired", "export_permission_unavailable")
        return
      }

      val source = File(sourceValue).canonicalFile
      validateSourceDirectory(source, directoryName, args.expectedFileCount, args.expectedSizeBytes)
      exportToTree(treeUri, source, directoryName)

      val result = JSObject()
      val label = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
        .getString(LABEL_KEY, null)
        ?.takeIf { it.isNotBlank() }
        ?: activity.getString(R.string.export_authorized_directory)
      result.put("destination", "$label/$directoryName")
      invoke.resolve(result)
    } catch (error: SecurityException) {
      clearStoredDestination()
      invoke.reject("The export directory permission has expired", "export_permission_unavailable", error)
    } catch (error: ExportConflictException) {
      invoke.reject("The target contains unrelated user data", "export_conflict", error)
    } catch (error: Exception) {
      invoke.reject("Unable to export the offline entry", "export_unavailable", error)
    }
  }

  private fun statusObject(): JSObject {
    val result = JSObject()
    val uri = currentTreeUri()
    val accessible = uri != null && hasPersistedAccess(uri)
    if (uri != null && !accessible) clearStoredDestination()
    val preferences = activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
    result.put("configured", accessible)
    result.put("accessible", accessible)
    if (accessible) {
      result.put(
        "label",
        preferences.getString(LABEL_KEY, null)?.takeIf { it.isNotBlank() }
          ?: activity.getString(R.string.export_authorized_directory),
      )
    }
    return result
  }

  private fun currentTreeUri(): Uri? =
    activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE)
      .getString(URI_KEY, null)
      ?.takeIf { it.isNotBlank() }
      ?.let(Uri::parse)

  private fun hasPersistedAccess(uri: Uri): Boolean =
    activity.contentResolver.persistedUriPermissions.any {
      it.uri == uri && it.isReadPermission && it.isWritePermission
    }

  private fun clearStoredDestination() {
    activity.getSharedPreferences(PREFERENCES, Context.MODE_PRIVATE).edit().clear().commit()
  }

  private fun displayName(treeUri: Uri): String {
    val rootDocument = DocumentsContract.buildDocumentUriUsingTree(
      treeUri,
      DocumentsContract.getTreeDocumentId(treeUri),
    )
    activity.contentResolver.query(
      rootDocument,
      arrayOf(OpenableColumns.DISPLAY_NAME),
      null,
      null,
      null,
    )?.use { cursor ->
      if (cursor.moveToFirst()) {
        val value = cursor.getString(0)?.trim()
        if (!value.isNullOrEmpty()) return value.take(80)
      }
    }
    return activity.getString(R.string.export_authorized_directory)
  }

  private fun validateSourceDirectory(
    source: File,
    directoryName: String,
    expectedFileCount: Int,
    expectedSizeBytes: Long,
  ) {
    val allowedRoot = File(activity.cacheDir, EXPORT_STAGING_DIRECTORY).canonicalFile
    check(source.parentFile?.canonicalFile == allowedRoot) { "Source is outside export staging" }
    check(source.name == directoryName && source.isDirectory) { "Invalid staging directory" }
    check(!Files.isSymbolicLink(source.toPath())) { "Staging directory is a symbolic link" }

    var fileCount = 0
    var totalBytes = 0L
    source.walkTopDown().forEach { entry ->
      check(entry.canonicalPath == entry.absolutePath) { "Symbolic links are not allowed" }
      val relative = entry.relativeTo(source)
      if (relative.path.isNotEmpty()) {
        check(FILE_NAME.matches(entry.name)) { "Invalid staged file name" }
      }
      if (entry.isFile) {
        fileCount += 1
        check(fileCount <= MAX_EXPORT_FILES) { "Too many staged files" }
        totalBytes = Math.addExact(totalBytes, entry.length())
      } else {
        check(entry.isDirectory) { "Unsupported staged entry" }
      }
    }
    check(fileCount == expectedFileCount) { "Staged file count changed" }
    check(totalBytes >= expectedSizeBytes) { "Staged files are incomplete" }
    check(totalBytes - expectedSizeBytes <= MAX_MARKER_BYTES) { "Unexpected staged payload" }
  }

  private fun exportToTree(treeUri: Uri, source: File, directoryName: String) {
    val resolver = activity.contentResolver
    val root = DocumentsContract.buildDocumentUriUsingTree(
      treeUri,
      DocumentsContract.getTreeDocumentId(treeUri),
    )
    val existing = findChild(treeUri, root, directoryName)
    if (existing != null) {
      if (
        existing.mimeType != DocumentsContract.Document.MIME_TYPE_DIR ||
        !isOwnedExportDirectory(treeUri, existing.uri, directoryName)
      ) {
        throw ExportConflictException()
      }
    }

    val temporaryName = "${directoryName}-temporary-${System.currentTimeMillis()}"
    val temporary = DocumentsContract.createDocument(
      resolver,
      root,
      DocumentsContract.Document.MIME_TYPE_DIR,
      temporaryName,
    ) ?: error("Unable to create temporary export directory")

    try {
      copyDirectory(source, temporary)
      if (existing != null && !DocumentsContract.deleteDocument(resolver, existing.uri)) {
        error("Unable to replace the previous export")
      }
      val renamed = DocumentsContract.renameDocument(resolver, temporary, directoryName)
      if (renamed == null) {
        val target = DocumentsContract.createDocument(
          resolver,
          root,
          DocumentsContract.Document.MIME_TYPE_DIR,
          directoryName,
        ) ?: error("Unable to create export directory")
        try {
          copyDirectory(source, target)
        } catch (error: Exception) {
          DocumentsContract.deleteDocument(resolver, target)
          throw error
        }
        DocumentsContract.deleteDocument(resolver, temporary)
      }
    } catch (error: Exception) {
      try {
        DocumentsContract.deleteDocument(resolver, temporary)
      } catch (_: Exception) {
        // The provider may have already removed or renamed the temporary document.
      }
      throw error
    }
  }

  private fun copyDirectory(source: File, destination: Uri) {
    source.listFiles()?.sortedBy { it.name }?.forEach { child ->
      if (child.isDirectory) {
        val created = DocumentsContract.createDocument(
          activity.contentResolver,
          destination,
          DocumentsContract.Document.MIME_TYPE_DIR,
          child.name,
        ) ?: error("Unable to create export subdirectory")
        copyDirectory(child, created)
      } else {
        val created = DocumentsContract.createDocument(
          activity.contentResolver,
          destination,
          mimeTypeFor(child.name),
          child.name,
        ) ?: error("Unable to create export file")
        val expectedBytes = child.length()
        val writtenBytes = activity.contentResolver.openOutputStream(created, "w")?.use { output ->
          FileInputStream(child).use { input -> input.copyTo(output, 128 * 1024) }
        } ?: error("Unable to open export file")
        check(writtenBytes == expectedBytes) { "Export file write was incomplete" }
        val verifiedBytes = activity.contentResolver.openInputStream(created)?.use { input ->
          var total = 0L
          val buffer = ByteArray(128 * 1024)
          while (true) {
            val read = input.read(buffer)
            if (read < 0) break
            total = Math.addExact(total, read.toLong())
          }
          total
        } ?: error("Unable to verify export file")
        check(verifiedBytes == expectedBytes) { "Export file verification failed" }
      }
    } ?: error("Unable to enumerate staged export")
  }

  private fun findChild(treeUri: Uri, parent: Uri, name: String): DocumentChild? {
    val parentId = DocumentsContract.getDocumentId(parent)
    val children = DocumentsContract.buildChildDocumentsUriUsingTree(treeUri, parentId)
    activity.contentResolver.query(
      children,
      arrayOf(
        DocumentsContract.Document.COLUMN_DOCUMENT_ID,
        DocumentsContract.Document.COLUMN_DISPLAY_NAME,
        DocumentsContract.Document.COLUMN_MIME_TYPE,
      ),
      null,
      null,
      null,
    )?.use { cursor ->
      while (cursor.moveToNext()) {
        if (cursor.getString(1) == name) {
          return DocumentChild(
            DocumentsContract.buildDocumentUriUsingTree(treeUri, cursor.getString(0)),
            cursor.getString(2),
          )
        }
      }
    }
    return null
  }

  private fun isOwnedExportDirectory(treeUri: Uri, directory: Uri, expectedKey: String): Boolean {
    val marker = findChild(treeUri, directory, EXPORT_MARKER_FILE) ?: return false
    if (marker.mimeType == DocumentsContract.Document.MIME_TYPE_DIR) return false
    return try {
      val bytes = activity.contentResolver.openInputStream(marker.uri)?.use { input ->
        val output = ByteArrayOutputStream()
        val buffer = ByteArray(16 * 1024)
        while (output.size() <= MAX_MARKER_BYTES) {
          val read = input.read(buffer)
          if (read < 0) break
          output.write(buffer, 0, read)
        }
        output.toByteArray()
      } ?: return false
      if (bytes.size > MAX_MARKER_BYTES) return false
      val entry = JSONObject(String(bytes, Charsets.UTF_8)).optJSONObject("entry") ?: return false
      entry.optString("key") == expectedKey
    } catch (_: Exception) {
      false
    }
  }

  private fun mimeTypeFor(name: String): String = when (name.substringAfterLast('.', "").lowercase()) {
    "jpg", "jpeg" -> "image/jpeg"
    "png" -> "image/png"
    "gif" -> "image/gif"
    "webp" -> "image/webp"
    "avif" -> "image/avif"
    "zip" -> "application/zip"
    "json" -> "application/json"
    "txt" -> "text/plain"
    else -> "application/octet-stream"
  }
}

private class ExportConflictException : Exception("Export directory conflict")
