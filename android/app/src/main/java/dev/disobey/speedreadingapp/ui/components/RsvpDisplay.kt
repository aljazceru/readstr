package dev.disobey.speedreadingapp.ui.components

import androidx.compose.foundation.layout.*
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.speedreadingapp.rust.WordDisplay

@Composable
fun RsvpDisplay(display: WordDisplay?, isLoading: Boolean) {
    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(120.dp),
        contentAlignment = Alignment.Center
    ) {
        when {
            display != null -> {
                val fullText = display.words.joinToString(" ") { it.before + it.anchor + it.after }
                val fontSize = when {
                    fullText.length <= 8  -> 48.sp
                    fullText.length <= 14 -> 38.sp
                    fullText.length <= 20 -> 30.sp
                    else                  -> 24.sp
                }
                val text = buildAnnotatedString {
                    display.words.forEachIndexed { i, seg ->
                        append(seg.before)
                        withStyle(
                            SpanStyle(
                                color = Color(0xFFFF6600),
                                fontWeight = FontWeight.Bold
                            )
                        ) {
                            append(seg.anchor)
                        }
                        append(seg.after)
                        if (i < display.words.size - 1) append(" ")
                    }
                }
                Text(
                    text = text,
                    fontSize = fontSize,
                    textAlign = TextAlign.Center
                )
            }
            isLoading -> Text("Loading...", fontSize = 48.sp)
            else -> Text("—", fontSize = 48.sp)
        }
    }
}
