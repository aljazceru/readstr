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

class AppManager private constructor(context: Context) : AppReconciler {
    private val mainHandler = Handler(Looper.getMainLooper())
    private val rust: FfiApp
    private var lastRevApplied: ULong = 0UL

    // NOTE: This placeholder is immediately replaced by the Rust core's
    // initial state snapshot in init{}. If you change AppState fields in
    // Rust, update this placeholder to match or the Kotlin code will not
    // compile -- this is intentional, ensuring the types stay in sync.
    var state: AppState by mutableStateOf(
        AppState(rev = 0UL, greeting = ""),
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
