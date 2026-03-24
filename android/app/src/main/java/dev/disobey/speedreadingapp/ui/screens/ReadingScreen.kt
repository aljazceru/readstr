package dev.disobey.speedreadingapp.ui.screens

import androidx.compose.foundation.layout.*
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.disobey.speedreadingapp.AppManager
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.ui.components.RsvpDisplay

@Composable
fun ReadingScreen(manager: AppManager) {
    val state = manager.state

    // Local preview states — update from core on state change (same pattern as iOS)
    var wpmPreview by remember { mutableFloatStateOf(state.wpm.toFloat()) }
    var groupPreview by remember { mutableFloatStateOf(state.wordsPerGroup.toFloat()) }
    var seekPreview by remember { mutableFloatStateOf(state.progressPercent) }

    // Sync preview values when core state updates (e.g., after Foregrounded restores session)
    LaunchedEffect(state.wpm) { wpmPreview = state.wpm.toFloat() }
    LaunchedEffect(state.wordsPerGroup) { groupPreview = state.wordsPerGroup.toFloat() }
    LaunchedEffect(state.progressPercent) {
        // Only sync seek if user is not dragging (approximated by checking if not playing)
        if (!state.isPlaying) seekPreview = state.progressPercent
    }

    val isFinished = state.progressPercent >= 99.9f && !state.isPlaying && state.totalWords > 0UL

    Column(
        modifier = Modifier
            .fillMaxSize()
            .padding(24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp)
    ) {
        // Back button
        TextButton(onClick = { manager.dispatch(AppAction.PopScreen) }) {
            Text("← Back")
        }

        // RSVP word display
        RsvpDisplay(display = state.display, isLoading = state.isLoading)

        // Progress seek bar
        Slider(
            value = seekPreview,
            onValueChange = { seekPreview = it },
            onValueChangeFinished = {
                manager.dispatch(AppAction.SeekToProgress(percent = seekPreview))
            },
            valueRange = 0f..100f,
            modifier = Modifier.fillMaxWidth()
        )

        // Play/pause + replay row
        Row(
            modifier = Modifier.fillMaxWidth(),
            horizontalArrangement = Arrangement.Center,
            verticalAlignment = Alignment.CenterVertically
        ) {
            Button(onClick = { manager.dispatch(AppAction.Toggle) }) {
                Text(if (state.isPlaying) "⏸ Pause" else "▶ Play")
            }
            if (isFinished) {
                Spacer(modifier = Modifier.width(12.dp))
                OutlinedButton(onClick = { manager.dispatch(AppAction.Replay) }) {
                    Text("↩ Replay")
                }
            }
        }

        // WPM slider
        Text("Speed: ${wpmPreview.toInt()} WPM", style = MaterialTheme.typography.bodyMedium)
        Slider(
            value = wpmPreview,
            onValueChange = { wpmPreview = it },
            onValueChangeFinished = {
                manager.dispatch(AppAction.SetWpm(wpm = wpmPreview.toUInt()))
            },
            valueRange = 100f..1000f,
            steps = 89,  // (1000-100)/10 - 1 = 89 intermediate steps of 10 WPM
            modifier = Modifier.fillMaxWidth()
        )

        // Words-per-group slider
        Text(
            "Words per group: ${groupPreview.toInt()}",
            style = MaterialTheme.typography.bodyMedium
        )
        Slider(
            value = groupPreview,
            onValueChange = { groupPreview = it },
            onValueChangeFinished = {
                manager.dispatch(
                    AppAction.SetWordsPerGroup(n = groupPreview.toUInt())
                )
            },
            valueRange = 1f..5f,
            steps = 3,  // 4 intervals: 1,2,3,4,5 → 3 intermediate steps
            modifier = Modifier.fillMaxWidth()
        )

        // Error/toast display
        state.error?.let { err ->
            Text(
                "Error: $err",
                color = MaterialTheme.colorScheme.error,
                style = MaterialTheme.typography.bodySmall
            )
        }
        state.toast?.let { msg ->
            Text(msg, style = MaterialTheme.typography.bodySmall)
        }
    }
}
