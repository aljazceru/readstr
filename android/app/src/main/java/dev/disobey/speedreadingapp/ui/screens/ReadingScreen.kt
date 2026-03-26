package dev.disobey.speedreadingapp.ui.screens

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Pause
import androidx.compose.material.icons.filled.PlayArrow
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.speedreadingapp.AppManager
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.ui.components.RsvpDisplay
import dev.disobey.speedreadingapp.ui.theme.AccentOrange

@Composable
fun ReadingScreen(manager: AppManager, darkTheme: Boolean = false, onToggleTheme: () -> Unit = {}) {
    val state = manager.state

    var wpmPreview by remember { mutableFloatStateOf(state.wpm.toFloat()) }
    var groupPreview by remember { mutableFloatStateOf(state.wordsPerGroup.toFloat()) }
    var seekPreview by remember { mutableFloatStateOf(state.progressPercent) }
    var isDragging by remember { mutableStateOf(false) }

    LaunchedEffect(state.wpm) { wpmPreview = state.wpm.toFloat() }
    LaunchedEffect(state.wordsPerGroup) { groupPreview = state.wordsPerGroup.toFloat() }
    LaunchedEffect(state.progressPercent) {
        if (!isDragging) seekPreview = state.progressPercent
    }

    val isFinished = state.progressPercent >= 99.9f && !state.isPlaying && state.totalWords > 0UL

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(horizontal = 20.dp),
    ) {
        Spacer(modifier = Modifier.height(16.dp))

        // ── Nav row ────────────────────────────────────────────────────────
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            IconButton(onClick = { manager.dispatch(AppAction.PopScreen) }) {
                Icon(
                    Icons.AutoMirrored.Filled.ArrowBack,
                    contentDescription = "Back",
                    tint = MaterialTheme.colorScheme.onSurfaceVariant,
                )
            }
            TextButton(onClick = onToggleTheme) {
                Text(
                    if (darkTheme) "light" else "dark",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                    letterSpacing = 0.08.sp,
                )
            }
        }

        Spacer(modifier = Modifier.height(24.dp))

        // ── RSVP stage ─────────────────────────────────────────────────────
        RsvpDisplay(display = state.display, isLoading = state.isLoading)

        Spacer(modifier = Modifier.height(24.dp))

        // ── Seek bar ───────────────────────────────────────────────────────
        Column {
            Slider(
                value = seekPreview,
                onValueChange = { isDragging = true; seekPreview = it },
                onValueChangeFinished = {
                    isDragging = false
                    manager.dispatch(AppAction.SeekToProgress(percent = seekPreview))
                },
                valueRange = 0f..100f,
                modifier = Modifier.fillMaxWidth(),
                colors = SliderDefaults.colors(
                    thumbColor = AccentOrange,
                    activeTrackColor = AccentOrange,
                    inactiveTrackColor = MaterialTheme.colorScheme.outlineVariant,
                )
            )
            Row(
                modifier = Modifier.fillMaxWidth(),
                horizontalArrangement = Arrangement.SpaceBetween
            ) {
                Text(
                    "${seekPreview.toInt()}%",
                    style = MaterialTheme.typography.labelSmall,
                    color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                )
                if (state.totalWords > 0UL) {
                    Text(
                        "${state.totalWords} words",
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.5f),
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(28.dp))

        // ── Play / Pause + Replay ──────────────────────────────────────────
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically,
            ) {
            if (isFinished) {
                TextButton(onClick = { manager.dispatch(AppAction.Replay) }) {
                    Text(
                        "↩ replay",
                        style = MaterialTheme.typography.labelMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                Spacer(modifier = Modifier.width(16.dp))
            }

            Box(
                modifier = Modifier
                    .size(64.dp)
                    .clip(CircleShape)
                    .background(AccentOrange),
                contentAlignment = Alignment.Center,
            ) {
                IconButton(onClick = { manager.dispatch(AppAction.Toggle) }) {
                    Icon(
                        imageVector = if (state.isPlaying) Icons.Default.Pause else Icons.Default.PlayArrow,
                        contentDescription = if (state.isPlaying) "Pause" else "Play",
                        tint = Color(0xFF1A0A00),
                        modifier = Modifier.size(32.dp),
                    )
                }
            }
        }

        Spacer(modifier = Modifier.height(32.dp))

        // ── Speed control ──────────────────────────────────────────────────
        SliderControl(
            label = "speed",
            value = wpmPreview,
            valueDisplay = "${wpmPreview.toInt()} wpm",
            onValueChange = { wpmPreview = it },
            onValueChangeFinished = {
                manager.dispatch(AppAction.SetWpm(wpm = wpmPreview.toUInt()))
            },
            valueRange = 100f..1000f,
            steps = 89,
        )

        Spacer(modifier = Modifier.height(16.dp))

        // ── Group control ──────────────────────────────────────────────────
        SliderControl(
            label = "group",
            value = groupPreview,
            valueDisplay = "${groupPreview.toInt()} words",
            onValueChange = { groupPreview = it },
            onValueChangeFinished = {
                manager.dispatch(AppAction.SetWordsPerGroup(n = groupPreview.toUInt()))
            },
            valueRange = 1f..5f,
            steps = 3,
        )

        // ── Error / toast ──────────────────────────────────────────────────
        state.error?.let {
            Spacer(modifier = Modifier.height(12.dp))
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.error)
        }
        state.toast?.let {
            Spacer(modifier = Modifier.height(12.dp))
            Text(it, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
        }
    }
}

@Composable
private fun SliderControl(
    label: String,
    value: Float,
    valueDisplay: String,
    onValueChange: (Float) -> Unit,
    onValueChangeFinished: () -> Unit,
    valueRange: ClosedFloatingPointRange<Float>,
    steps: Int,
) {
    Column {
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.SpaceBetween,
            verticalAlignment = Alignment.CenterVertically,
        ) {
            Text(
                label,
                style = MaterialTheme.typography.labelSmall,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.6f),
                letterSpacing = 0.1.sp,
            )
            Text(
                valueDisplay,
                style = MaterialTheme.typography.labelSmall,
                color = AccentOrange,
                fontWeight = FontWeight.Medium,
            )
        }
        Slider(
            value = value,
            onValueChange = onValueChange,
            onValueChangeFinished = onValueChangeFinished,
            valueRange = valueRange,
            steps = steps,
            modifier = Modifier.fillMaxWidth(),
            colors = SliderDefaults.colors(
                thumbColor = AccentOrange,
                activeTrackColor = AccentOrange,
                inactiveTrackColor = MaterialTheme.colorScheme.outlineVariant,
            )
        )
    }
}
