package dev.disobey.speedreadingapp.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.runtime.Composable
import dev.disobey.speedreadingapp.AppManager
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.rust.Screen
import dev.disobey.speedreadingapp.ui.screens.LandingScreen
import dev.disobey.speedreadingapp.ui.screens.ReadingScreen

@Composable
fun MainApp(manager: AppManager, onToggleTheme: () -> Unit = {}) {
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
                onToggleTheme = onToggleTheme
            )
        }
    }
}
