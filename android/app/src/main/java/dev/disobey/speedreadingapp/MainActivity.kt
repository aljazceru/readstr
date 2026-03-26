package dev.disobey.speedreadingapp

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Surface
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.ui.MainApp
import dev.disobey.speedreadingapp.ui.theme.AppTheme
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

private val android.content.Context.dataStore: DataStore<Preferences>
    by preferencesDataStore(name = "settings")

private val DARK_MODE_KEY = booleanPreferencesKey("dark_mode")

class MainActivity : ComponentActivity() {
    private var manager: AppManager? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        setContent {
            val scope = rememberCoroutineScope()
            // Start with false (light); DataStore emits the real value almost immediately
            val darkMode by applicationContext.dataStore.data
                .map { prefs -> prefs[DARK_MODE_KEY] ?: false }
                .collectAsState(initial = false)

            // Initialize Rust core off the main thread
            var readyManager by remember { mutableStateOf(manager) }
            LaunchedEffect(Unit) {
                if (readyManager == null) {
                    readyManager = withContext(Dispatchers.Default) {
                        AppManager.getInstance(applicationContext)
                    }
                    manager = readyManager
                }
            }

            AppTheme(darkTheme = darkMode) {
                Surface(
                    modifier = Modifier.fillMaxSize(),
                    color = MaterialTheme.colorScheme.background
                ) {
                    val m = readyManager
                    if (m != null) {
                        MainApp(
                            manager = m,
                            darkTheme = darkMode,
                            onToggleTheme = {
                                scope.launch {
                                    applicationContext.dataStore.edit { prefs ->
                                        prefs[DARK_MODE_KEY] = !(prefs[DARK_MODE_KEY] ?: false)
                                    }
                                }
                            }
                        )
                    } else {
                        Box(modifier = Modifier.fillMaxSize())
                    }
                }
            }
        }
    }

    override fun onPause() {
        super.onPause()
        manager?.dispatch(AppAction.BackgroundPause)
    }

    override fun onResume() {
        super.onResume()
        manager?.dispatch(AppAction.Foregrounded)
    }
}
