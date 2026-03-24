package dev.disobey.speedreadingapp

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.*
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.booleanPreferencesKey
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.preferencesDataStore
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.ui.MainApp
import dev.disobey.speedreadingapp.ui.theme.AppTheme
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch

private val android.content.Context.dataStore: DataStore<Preferences>
    by preferencesDataStore(name = "settings")

private val DARK_MODE_KEY = booleanPreferencesKey("dark_mode")

class MainActivity : ComponentActivity() {
    private lateinit var manager: AppManager

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        manager = AppManager.getInstance(applicationContext)

        setContent {
            val scope = rememberCoroutineScope()
            val darkMode by applicationContext.dataStore.data
                .map { prefs -> prefs[DARK_MODE_KEY] ?: false }
                .collectAsState(initial = false)

            AppTheme(darkTheme = darkMode) {
                MainApp(
                    manager = manager,
                    onToggleTheme = {
                        scope.launch {
                            applicationContext.dataStore.edit { prefs ->
                                prefs[DARK_MODE_KEY] = !(prefs[DARK_MODE_KEY] ?: false)
                            }
                        }
                    }
                )
            }
        }
    }

    override fun onPause() {
        super.onPause()
        manager.dispatch(AppAction.Pause)
    }

    override fun onResume() {
        super.onResume()
        manager.dispatch(AppAction.Foregrounded)
    }
}
