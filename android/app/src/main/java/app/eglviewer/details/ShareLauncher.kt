package app.eglviewer.details

import android.content.Context
import android.content.Intent
import androidx.core.content.FileProvider

/** Builds a system share intent for prepared originals (A3 ≈ D-095). */
object ShareLauncher {
    fun intent(context: Context, files: ShareFiles): Intent {
        val uris = ArrayList(
            files.files.map { file ->
                FileProvider.getUriForFile(
                    context,
                    "${context.packageName}.share",
                    file,
                )
            },
        )
        return Intent().apply {
            action = if (uris.size == 1) Intent.ACTION_SEND else Intent.ACTION_SEND_MULTIPLE
            type = "*/*"
            if (uris.size == 1) {
                putExtra(Intent.EXTRA_STREAM, uris[0])
            } else {
                putParcelableArrayListExtra(Intent.EXTRA_STREAM, uris)
            }
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }.let { Intent.createChooser(it, null) }
    }
}
