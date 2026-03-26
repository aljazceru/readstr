package dev.disobey.readstr.ui.screens

import android.content.Context
import android.net.Uri
import android.provider.OpenableColumns
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.background
import androidx.compose.foundation.border
import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.BasicTextField
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.filled.Delete
import androidx.compose.material.icons.filled.Description
import androidx.compose.material.icons.filled.Warning
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.readstr.AppManager
import dev.disobey.readstr.rust.AppAction
import dev.disobey.readstr.rust.Screen
import dev.disobey.readstr.ui.theme.AccentOrange
import dev.disobey.readstr.ui.theme.SyneFontFamily
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
        val path = copySandbox(context, uri)
        if (path != null) {
            fileNotFoundError = null
            manager.dispatch(AppAction.PushScreen(screen = Screen.READING))
            manager.dispatch(AppAction.FileSelected(path = path))
        }
    }

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 24.dp),
    ) {
        Spacer(modifier = Modifier.height(52.dp))

        // ── Brand ──────────────────────────────────────────────────────────
        Text(
            text = buildAnnotatedString {
                withStyle(SpanStyle(color = MaterialTheme.colorScheme.onBackground)) {
                    append("read")
                }
                withStyle(SpanStyle(color = AccentOrange)) {
                    append("str")
                }
            },
            fontFamily = SyneFontFamily,
            fontWeight = FontWeight.Bold,
            fontSize = 32.sp,
            letterSpacing = (-0.5).sp,
        )

        Text(
            text = "speed reading, focused",
            style = MaterialTheme.typography.bodySmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant,
            letterSpacing = 0.05.sp,
        )

        Spacer(modifier = Modifier.height(36.dp))

        // ── Paste area ─────────────────────────────────────────────────────
        Box(
            modifier = Modifier
                .fillMaxWidth()
                .clip(RoundedCornerShape(10.dp))
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .border(
                    width = 1.dp,
                    color = MaterialTheme.colorScheme.outlineVariant,
                    shape = RoundedCornerShape(10.dp)
                )
        ) {
            BasicTextField(
                value = pasteText,
                onValueChange = { pasteText = it },
                textStyle = MaterialTheme.typography.bodyMedium.copy(
                    color = MaterialTheme.colorScheme.onSurface,
                    lineHeight = 22.sp,
                ),
                modifier = Modifier
                    .fillMaxWidth()
                    .height(160.dp)
                    .padding(16.dp),
                decorationBox = { inner ->
                    if (pasteText.isEmpty()) {
                        Text(
                            "paste text to read…",
                            style = MaterialTheme.typography.bodyMedium,
                            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                        )
                    }
                    inner()
                }
            )
        }

        Spacer(modifier = Modifier.height(12.dp))

        // ── Action buttons ─────────────────────────────────────────────────
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.spacedBy(10.dp)
        ) {
            Button(
                onClick = {
                    if (pasteText.isNotBlank()) {
                        manager.dispatch(AppAction.PushScreen(screen = Screen.READING))
                        manager.dispatch(AppAction.LoadText(text = pasteText))
                    }
                },
                modifier = Modifier.weight(1f).height(48.dp),
                shape = RoundedCornerShape(10.dp),
                colors = ButtonDefaults.buttonColors(
                    containerColor = AccentOrange,
                    contentColor = Color(0xFF1A0A00),
                ),
                enabled = pasteText.isNotBlank(),
            ) {
                Text(
                    "Read",
                    fontWeight = FontWeight.Bold,
                    fontSize = 15.sp,
                    letterSpacing = 0.03.sp,
                )
            }

            OutlinedButton(
                onClick = {
                    fileLauncher.launch(
                        arrayOf("text/plain", "application/epub+zip", "application/pdf")
                    )
                },
                modifier = Modifier.weight(1f).height(48.dp),
                shape = RoundedCornerShape(10.dp),
                border = ButtonDefaults.outlinedButtonBorder.copy(
                    width = 1.dp,
                ),
                colors = ButtonDefaults.outlinedButtonColors(
                    contentColor = MaterialTheme.colorScheme.onSurfaceVariant,
                ),
            ) {
                Text(
                    "Open file",
                    fontSize = 15.sp,
                )
            }
        }

        // ── Status messages ────────────────────────────────────────────────
        val errorText = when {
            state.error != null -> state.error
            fileNotFoundError != null -> fileNotFoundError
            else -> null
        }
        if (state.isLoading || errorText != null) {
            Spacer(modifier = Modifier.height(10.dp))
            Text(
                text = if (state.isLoading) "loading…" else errorText ?: "",
                style = MaterialTheme.typography.bodySmall,
                color = if (state.isLoading)
                    MaterialTheme.colorScheme.onSurfaceVariant
                else
                    MaterialTheme.colorScheme.error,
                letterSpacing = 0.03.sp,
            )
        }

        Spacer(modifier = Modifier.height(32.dp))

        // ── History ────────────────────────────────────────────────────────
        HistorySection(
            history = manager.history,
            onResume = { item ->
                if (item.isMissing) {
                    fileNotFoundError = "file not found — tap to re-locate"
                    relocateLauncher.launch(
                        arrayOf("text/plain", "application/epub+zip", "application/pdf")
                    )
                } else {
                    fileNotFoundError = null
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
            containerColor = MaterialTheme.colorScheme.surface,
            title = {
                Text(
                    "Remove entry?",
                    style = MaterialTheme.typography.titleMedium,
                )
            },
            text = {
                Text(
                    pendingDeleteEntry!!.entry.fileName,
                    style = MaterialTheme.typography.bodySmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    maxLines = 2,
                    overflow = TextOverflow.Ellipsis,
                )
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
                    Text("Remove", color = MaterialTheme.colorScheme.error)
                }
            },
            dismissButton = {
                TextButton(onClick = {
                    showDeleteConfirm = false
                    pendingDeleteEntry = null
                }) {
                    Text("Keep")
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
    if (history.isEmpty()) return

    Column {
        Text(
            "recent",
            style = MaterialTheme.typography.labelSmall,
            color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
            letterSpacing = 0.12.sp,
        )
        Spacer(modifier = Modifier.height(10.dp))
        LazyColumn(verticalArrangement = Arrangement.spacedBy(2.dp)) {
            items(history, key = { it.entry.fileHash }) { item ->
                HistoryRow(item = item, onResume = onResume, onDeleteRequest = onDeleteRequest)
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
            .clip(RoundedCornerShape(8.dp))
            .clickable { onResume(item) }
            .padding(horizontal = 12.dp, vertical = 10.dp),
        verticalAlignment = Alignment.CenterVertically,
        horizontalArrangement = Arrangement.spacedBy(10.dp)
    ) {
        // Icon
        Icon(
            imageVector = if (item.isMissing) Icons.Default.Warning else Icons.Default.Description,
            contentDescription = null,
            tint = if (item.isMissing)
                MaterialTheme.colorScheme.error.copy(alpha = 0.7f)
            else
                MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
            modifier = Modifier.size(18.dp)
        )

        // Name + sublabel
        Column(modifier = Modifier.weight(1f)) {
            Text(
                text = item.entry.fileName,
                style = MaterialTheme.typography.bodyMedium,
                color = if (item.isMissing)
                    MaterialTheme.colorScheme.onSurfaceVariant
                else
                    MaterialTheme.colorScheme.onSurface,
                maxLines = 1,
                overflow = TextOverflow.Ellipsis,
            )
            if (item.isMissing) {
                Text(
                    "file not found",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.error.copy(alpha = 0.7f),
                )
            }
        }

        // Progress pill
        Box(
            modifier = Modifier
                .clip(CircleShape)
                .background(MaterialTheme.colorScheme.surfaceVariant)
                .padding(horizontal = 8.dp, vertical = 3.dp),
            contentAlignment = Alignment.Center
        ) {
            Text(
                text = "${item.entry.progressPercent.toInt()}%",
                style = MaterialTheme.typography.labelSmall,
                color = if (item.entry.progressPercent > 0f)
                    AccentOrange
                else
                    MaterialTheme.colorScheme.onSurfaceVariant,
                fontWeight = FontWeight.Medium,
            )
        }

        // Delete
        IconButton(
            onClick = { onDeleteRequest(item) },
            modifier = Modifier.size(36.dp)
        ) {
            Icon(
                Icons.Default.Delete,
                contentDescription = "Remove",
                tint = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.35f),
                modifier = Modifier.size(18.dp)
            )
        }
    }
}
