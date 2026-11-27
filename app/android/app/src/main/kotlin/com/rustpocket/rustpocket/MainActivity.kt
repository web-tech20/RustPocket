package com.rustpocket.rustpocket

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel

class MainActivity : FlutterActivity() {
    companion object {
        private const val CHANNEL = "com.rustpocket/files"
        private const val REQ_OPEN_FILE = 1001
        private const val REQ_SAVE_FILE = 1002
    }

    private var pendingResult: MethodChannel.Result? = null
    private var pendingSaveContent: String? = null

    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, CHANNEL)
            .setMethodCallHandler { call, result ->
                if (pendingResult != null) {
                    result.error("BUSY", "Une opération sur un fichier est déjà en cours.", null)
                    return@setMethodCallHandler
                }
                when (call.method) {
                    "openFile" -> {
                        pendingResult = result
                        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                            addCategory(Intent.CATEGORY_OPENABLE)
                            type = "*/*"
                        }
                        startActivityForResult(intent, REQ_OPEN_FILE)
                    }
                    "saveFileAs" -> {
                        val fileName = call.argument<String>("name") ?: "main.rs"
                        val content = call.argument<String>("content") ?: ""
                        pendingResult = result
                        pendingSaveContent = content
                        val intent = Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                            addCategory(Intent.CATEGORY_OPENABLE)
                            type = "text/plain"
                            putExtra(Intent.EXTRA_TITLE, fileName)
                        }
                        startActivityForResult(intent, REQ_SAVE_FILE)
                    }
                    else -> result.notImplemented()
                }
            }
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val result = pendingResult ?: return
        if (requestCode != REQ_OPEN_FILE && requestCode != REQ_SAVE_FILE) return

        pendingResult = null
        val contentToWrite = pendingSaveContent
        pendingSaveContent = null

        if (resultCode != Activity.RESULT_OK || data?.data == null) {
            result.success(null)
            return
        }

        val uri: Uri = data.data!!
        try {
            if (requestCode == REQ_OPEN_FILE) {
                val text = contentResolver.openInputStream(uri)?.use {
                    it.bufferedReader(Charsets.UTF_8).readText()
                } ?: ""
                val name = queryDisplayName(uri) ?: "external.rs"
                result.success(mapOf("name" to name, "content" to text))
            } else {
                contentResolver.openOutputStream(uri, "wt")?.use { out ->
                    out.write((contentToWrite ?: "").toByteArray(Charsets.UTF_8))
                }
                val name = queryDisplayName(uri) ?: "saved.rs"
                result.success(name)
            }
        } catch (e: Exception) {
            result.error("IO_ERROR", e.message ?: "Erreur d'accès au fichier.", null)
        }
    }

    private fun queryDisplayName(uri: Uri): String? {
        contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
            ?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val idx = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    if (idx >= 0) return cursor.getString(idx)
                }
            }
        return uri.lastPathSegment?.substringAfterLast('/')
    }
}
