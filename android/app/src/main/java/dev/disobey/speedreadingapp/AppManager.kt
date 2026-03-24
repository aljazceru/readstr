package dev.disobey.speedreadingapp

import android.content.Context
import android.os.Handler
import android.os.Looper
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.rust.AppReconciler
import dev.disobey.speedreadingapp.rust.AppState
import dev.disobey.speedreadingapp.rust.AppUpdate
import dev.disobey.speedreadingapp.rust.FfiApp
import dev.disobey.speedreadingapp.rust.Router
import dev.disobey.speedreadingapp.rust.Screen

class AppManager private constructor(context: Context) : AppReconciler {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val rust: FfiApp
    private var lastRevApplied: ULong = 0UL

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
                }
                is AppUpdate.PlaybackTick -> {
                    val latest = rust.state()
                    if (latest.rev >= lastRevApplied) {
                        lastRevApplied = latest.rev
                        state = latest
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
