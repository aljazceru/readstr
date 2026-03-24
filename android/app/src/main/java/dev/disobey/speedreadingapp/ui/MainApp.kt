package dev.disobey.speedreadingapp.ui

import androidx.activity.compose.BackHandler
import androidx.compose.animation.AnimatedContent
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import dev.disobey.speedreadingapp.AppManager
import dev.disobey.speedreadingapp.rust.AppAction
import dev.disobey.speedreadingapp.rust.Screen

@Composable
fun MainApp(manager: AppManager) {
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
            Screen.READING -> ReadingScreen(manager = manager)
        }
    }
}

// Temporary stubs — replaced by Plans 02 and 03
@Composable
fun LandingScreen(manager: AppManager) {
    Text("Landing — coming soon")
}

@Composable
fun ReadingScreen(manager: AppManager) {
    Text("Reading — coming soon")
}
