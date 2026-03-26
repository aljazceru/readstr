package dev.disobey.readstr.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.runtime.Composable
import dev.disobey.readstr.AppManager
import dev.disobey.readstr.rust.AppAction
import dev.disobey.readstr.rust.Screen
import dev.disobey.readstr.ui.screens.LandingScreen
import dev.disobey.readstr.ui.screens.ReadingScreen

@Composable
fun MainApp(manager: AppManager, darkTheme: Boolean = false, onToggleTheme: () -> Unit = {}) {
    val state = manager.state
    val router = state.router
    val currentScreen = router.screenStack.lastOrNull() ?: router.defaultScreen

    BackHandler(enabled = router.screenStack.isNotEmpty()) {
        manager.dispatch(AppAction.PopScreen)
    }

    AnimatedContent(
        targetState = currentScreen,
        label = "screen_navigation"
    ) { screen ->
        when (screen) {
            Screen.LANDING -> LandingScreen(manager = manager)
            Screen.READING -> ReadingScreen(
                manager = manager,
                darkTheme = darkTheme,
                onToggleTheme = onToggleTheme
            )
        }
    }
}
