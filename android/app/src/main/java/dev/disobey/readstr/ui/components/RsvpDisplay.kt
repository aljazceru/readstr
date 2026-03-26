package dev.disobey.readstr.ui.components

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.SpanStyle
import androidx.compose.ui.text.buildAnnotatedString
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.text.withStyle
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import dev.disobey.readstr.rust.WordDisplay
import dev.disobey.readstr.ui.theme.AccentOrange
import dev.disobey.readstr.ui.theme.JetBrainsMonoFontFamily

@Composable
fun RsvpDisplay(display: WordDisplay?, isLoading: Boolean) {
    val stageColor = MaterialTheme.colorScheme.surfaceVariant

    Box(
        modifier = Modifier
            .fillMaxWidth()
            .height(160.dp)
            .clip(RoundedCornerShape(12.dp))
            .background(stageColor)
            .padding(horizontal = 28.dp, vertical = 16.dp),
        contentAlignment = Alignment.Center
    ) {
        when {
            display != null -> {
                val fullText = display.words.joinToString(" ") { it.before + it.anchor + it.after }
                val fontSize = when {
                    fullText.length <= 5  -> 58.sp
                    fullText.length <= 9  -> 48.sp
                    fullText.length <= 14 -> 38.sp
                    fullText.length <= 20 -> 30.sp
                    else                  -> 24.sp
                }

                val text = buildAnnotatedString {
                    display.words.forEachIndexed { i, seg ->
                        withStyle(SpanStyle(color = MaterialTheme.colorScheme.onSurface)) {
                            append(seg.before)
                        }
                        withStyle(
                            SpanStyle(
                                color = AccentOrange,
                                fontWeight = FontWeight.Bold,
                            )
                        ) {
                            append(seg.anchor)
                        }
                        withStyle(SpanStyle(color = MaterialTheme.colorScheme.onSurface)) {
                            append(seg.after)
                        }
                        if (i < display.words.size - 1) append(" ")
                    }
                }

                Text(
                    text = text,
                    fontSize = fontSize,
                    fontFamily = JetBrainsMonoFontFamily,
                    textAlign = TextAlign.Center,
                    lineHeight = (fontSize.value * 1.2f).sp,
                )
            }

            isLoading -> Text(
                "loading",
                fontSize = 18.sp,
                fontFamily = JetBrainsMonoFontFamily,
                color = MaterialTheme.colorScheme.onSurfaceVariant,
                letterSpacing = 0.15.sp,
            )

            else -> Text(
                "—",
                fontSize = 40.sp,
                fontFamily = JetBrainsMonoFontFamily,
                color = MaterialTheme.colorScheme.onSurfaceVariant.copy(alpha = 0.4f),
            )
        }
    }
}
