package dev.disobey.readstr

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.disobey.readstr.rust.AppAction
import dev.disobey.readstr.rust.AppReconciler
import dev.disobey.readstr.rust.AppState
import dev.disobey.readstr.rust.AppUpdate
import dev.disobey.readstr.rust.FfiApp
import dev.disobey.readstr.rust.HistoryEntry
import dev.disobey.readstr.rust.Router
import dev.disobey.readstr.rust.Screen

class AppManager private constructor(context: Context) : AppReconciler {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val rust: FfiApp
    private var lastRevApplied: ULong = 0UL
    var history: List<HistoryEntryUi> by mutableStateOf(emptyList())
        private set
    private var lastHistoryRevision: ULong = 0UL

    data class HistoryEntryUi(
        val entry: HistoryEntry,
        val isMissing: Boolean
    )

    // Placeholder initial state — immediately replaced by rust.state() in init{}.
    // Required because mutableStateOf needs a typed initial value and AppState has no no-arg constructor.
    var state: AppState by mutableStateOf(
        AppState(
            rev = 0UL,
            router = Router(defaultScreen = Screen.LANDING, screenStack = emptyList()),
            display = null,
            wpm = 300u,
            wordsPerGroup = 1u,
            isPlaying = false,
            progressPercent = 0f,
            currentWordIndex = 0UL,
            totalWords = 0UL,
            isLoading = false,
            error = null,
            toast = null,
            historyRevision = 0UL,
        )
    )
        private set

    init {
        val dataDir = context.filesDir.absolutePath
        rust = FfiApp(dataDir)
        val initial = rust.state()
        state = initial
        lastRevApplied = initial.rev
        rust.listenForUpdates(this)
    }

    fun dispatch(action: AppAction) {
        rust.dispatch(action)
    }

    override fun reconcile(update: AppUpdate) {
        mainHandler.post {
            when (update) {
                is AppUpdate.FullState -> {
                    if (update.v1.rev <= lastRevApplied) return@post
                    lastRevApplied = update.v1.rev
                    state = update.v1
                    if (update.v1.historyRevision != lastHistoryRevision) {
                        lastHistoryRevision = update.v1.historyRevision
                        history = rust.getHistory().map { entry ->
                            HistoryEntryUi(entry, isMissing = !java.io.File(entry.filePath).exists())
                        }
                    }
                }
                is AppUpdate.PlaybackTick -> {
                    val latest = rust.state()
                    if (latest.rev >= lastRevApplied) {
                        lastRevApplied = latest.rev
                        state = latest
                    }
                    if (latest.historyRevision != lastHistoryRevision) {
                        lastHistoryRevision = latest.historyRevision
                        history = rust.getHistory().map { entry ->
                            HistoryEntryUi(entry, isMissing = !java.io.File(entry.filePath).exists())
                        }
                    }
                }
            }
        }
    }

    companion object {
        @Volatile
        private var instance: AppManager? = null

        fun getInstance(context: Context): AppManager =
            instance ?: synchronized(this) {
                instance ?: AppManager(context.applicationContext).also { instance = it }
            }
    }
}
