package dev.disobey.speedreadingapp.ui.screens

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp
import dev.disobey.speedreadingapp.AppManager
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.rust.Screen
import java.io.File

@Composable
fun LandingScreen(manager: AppManager) {
    val context = LocalContext.current
    var pasteText by remember { mutableStateOf("") }
    val state = manager.state

    val fileLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        val path = copySandbox(context, uri)
        if (path != null) {
            manager.dispatch(AppAction.PushScreen(screen = Screen.READING))
            manager.dispatch(AppAction.FileSelected(path = path))
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(32.dp),
        verticalArrangement = Arrangement.spacedBy(16.dp)
    ) {
        Text("SpeedReader", style = MaterialTheme.typography.headlineLarge)

        OutlinedTextField(
            value = pasteText,
            onValueChange = { pasteText = it },
            placeholder = { Text("Paste text here to start reading...") },
            modifier = Modifier
                .fillMaxWidth()
                .height(180.dp),
            maxLines = 8
        )

        Row(horizontalArrangement = Arrangement.spacedBy(12.dp)) {
            Button(onClick = {
                if (pasteText.isNotBlank()) {
                    manager.dispatch(AppAction.PushScreen(screen = Screen.READING))
                    manager.dispatch(AppAction.LoadText(text = pasteText))
                }
            }) {
                Text("Start Reading")
            }

            OutlinedButton(onClick = {
                fileLauncher.launch(
                    arrayOf("text/plain", "application/epub+zip", "application/pdf")
                )
            }) {
                Text("Open File")
            }
        }

        when {
            state.isLoading -> Text(
                "Loading file...",
                style = MaterialTheme.typography.bodySmall
            )
            state.error != null -> Text(
                "Error: ${state.error}",
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall
            )
        }
    }
}

fun copySandbox(context: Context, uri: Uri): String? {
    val fileName = context.contentResolver.query(
        uri, null, null, null, null
    )?.use { cursor ->
        val nameIndex = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
        cursor.moveToFirst()
        if (nameIndex >= 0) cursor.getString(nameIndex) else "imported_file"
    } ?: "imported_file"

    val importsDir = File(context.filesDir, "imports").also { it.mkdirs() }
    val destFile = File(importsDir, fileName)

    return try {
        context.contentResolver.openInputStream(uri)?.use { input ->
            destFile.outputStream().use { output ->
                input.copyTo(output)
            }
        }
        destFile.absolutePath
    } catch (e: Exception) {
        null
    }
}
