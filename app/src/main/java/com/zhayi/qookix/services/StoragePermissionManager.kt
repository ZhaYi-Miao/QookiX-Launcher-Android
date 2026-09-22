package com.zhayi.qookix.services

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Environment
import android.provider.Settings
import androidx.core.content.ContextCompat
import com.zhayi.qookix.QookixApplication

class StoragePermissionManager {

    fun hasStoragePermission(context: Context): Boolean {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+ - check for manage external storage permission
            Environment.isExternalStorageManager()
        } else {
            // Android 10 and below - check for legacy storage permission
            ContextCompat.checkSelfPermission(
                context,
                android.Manifest.permission.READ_EXTERNAL_STORAGE
            ) == android.content.pm.PackageManager.PERMISSION_GRANTED
        }
    }

    fun requestStoragePermission(context: Context): Intent? {
        return if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            // Android 11+ - open system settings for manage external storage
            Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION).apply {
                data = Uri.parse("package:${context.packageName}")
            }
        } else {
            // Android 10 and below - request runtime permission
            Intent(android.content.Intent.ACTION_OPEN_DOCUMENT_TREE)
        }
    }

    fun getGameDirectoryIntent(context: Context): Intent {
        // Use Storage Access Framework for Android 10+
        return Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            addFlags(Intent.FLAG_GRANT_WRITE_URI_PERMISSION)
        }
    }
}
