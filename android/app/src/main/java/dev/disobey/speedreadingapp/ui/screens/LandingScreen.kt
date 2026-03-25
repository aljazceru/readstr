package dev.disobey.speedreadingapp.ui.screens

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextOverflow
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
    var showDeleteConfirm by remember { mutableStateOf(false) }
    var pendingDeleteEntry by remember { mutableStateOf<AppManager.HistoryEntryUi?>(null) }
    var fileNotFoundError by remember { mutableStateOf<String?>(null) }

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

    val relocateLauncher = rememberLauncherForActivityResult(
        contract = ActivityResultContracts.OpenDocument()
    ) { uri ->
        uri ?: return@rememberLauncherForActivityResult
        // On cancel: uri is null, return early — fileNotFoundError stays visible (D-10)
        val path = copySandbox(context, uri)
        if (path != null) {
            fileNotFoundError = null  // Clear error only on successful pick
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
            fileNotFoundError != null -> Text(
                fileNotFoundError!!,
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall
            )
        }

        HistorySection(
            history = manager.history,
            onResume = { item ->
                if (item.isMissing) {
                    // D-10: show error + open re-locate picker; do NOT dispatch ResumeFile
                    fileNotFoundError = "File not found — please re-locate it"
                    relocateLauncher.launch(
                        arrayOf("text/plain", "application/epub+zip", "application/pdf")
                    )
                } else {
                    fileNotFoundError = null
                    // Do NOT dispatch PushScreen — on_parse_complete pushes Screen.READING (Pitfall 1)
                    manager.dispatch(AppAction.ResumeFile(fileHash = item.entry.fileHash))
                }
            },
            onDeleteRequest = { item ->
                pendingDeleteEntry = item
                showDeleteConfirm = true
            }
        )
    }

    if (showDeleteConfirm && pendingDeleteEntry != null) {
        AlertDialog(
            onDismissRequest = {
                showDeleteConfirm = false
                pendingDeleteEntry = null
            },
            title = {
                Text("Delete entry for ${pendingDeleteEntry!!.entry.fileName}?")
            },
            confirmButton = {
                TextButton(
                    onClick = {
                        manager.dispatch(
                            AppAction.DeleteSession(fileHash = pendingDeleteEntry!!.entry.fileHash)
                        )
                        showDeleteConfirm = false
                        pendingDeleteEntry = null
                    }
                ) {
                    Text("Delete", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(
                    onClick = {
                        showDeleteConfirm = false
                        pendingDeleteEntry = null
                    }
                ) {
                    Text("Keep Entry")
                }
            }
        )
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

@Composable
fun HistorySection(
    history: List<AppManager.HistoryEntryUi>,
    onResume: (AppManager.HistoryEntryUi) -> Unit,
    onDeleteRequest: (AppManager.HistoryEntryUi) -> Unit
) {
    if (history.isEmpty()) return  // D-03: hidden when empty, no header

    Column(verticalArrangement = Arrangement.spacedBy(0.dp)) {
        Text(
            "Recent Files",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant
        )
        Spacer(modifier = Modifier.height(8.dp))
        LazyColumn {
            items(history, key = { it.entry.fileHash }) { item ->
                HistoryRow(item = item, onResume = onResume, onDeleteRequest = onDeleteRequest)
                HorizontalDivider()
            }
        }
    }
}

@Composable
fun HistoryRow(
    item: AppManager.HistoryEntryUi,
    onResume: (AppManager.HistoryEntryUi) -> Unit,
    onDeleteRequest: (AppManager.HistoryEntryUi) -> Unit
) {
    Row(
        modifier = Modifier
            .fillMaxWidth()
            .padding(vertical = 8.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(8.dp)
    ) {
        // File icon — warning for missing (D-08, D-11)
        if (item.isMissing) {
            Icon(
                Icons.Default.Warning,
                contentDescription = "File not found",
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        } else {
            // Generic document icon — no system equivalent in Material Icons; use text fallback
            Text("📄", style = MaterialTheme.typography.bodyMedium)
        }

        // File name + optional sublabel (D-08)
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.entry.fileName,
                style = MaterialTheme.typography.bodyMedium,
                color = if (item.isMissing)
                    MaterialTheme.colorScheme.onSurfaceVariant
                else
                    MaterialTheme.colorScheme.onBackground,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis
            )
            if (item.isMissing) {
                Text(
                    text = "File not found",
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant
                )
            }
        }

        // Progress % — integer, no decimal (UI-SPEC Pitfall 6)
        Text(
            text = "${item.entry.progressPercent.toInt()}%",
            style = MaterialTheme.typography.bodySmall
        )

        // Resume button (D-12)
        Button(
            onClick = { onResume(item) },
            modifier = Modifier.heightIn(min = 44.dp)
        ) {
            Text("Resume")
        }

        // Trash icon — delete request (D-06)
        IconButton(
            onClick = { onDeleteRequest(item) },
            modifier = Modifier.size(44.dp)
        ) {
            Icon(
                Icons.Default.Delete,
                contentDescription = "Delete ${item.entry.fileName} history entry",
                tint = MaterialTheme.colorScheme.onSurfaceVariant
            )
        }
    }
}
