package dev.dioxus.main

import android.graphics.Color
import android.graphics.drawable.ColorDrawable
import android.os.Bundle
import android.view.View
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat

typealias BuildConfig = co.dipity.glpatlas.BuildConfig

/**
 * Top of the sky, matching `--sky-top` in the stylesheet.
 *
 * The window and the WebView both carry it. A WebView paints white until its page does, which
 * between the launch frame and the first render would otherwise flash against a dark app.
 */
private val SKY = Color.parseColor("#1E2242")

/** Asks the app to step back, reporting whether it had anywhere to go. */
private const val HANDLE_BACK = """
    (() => {
      const app = document.querySelector('.app');
      if (app?.dataset.back !== '1' || !window.__atlasBack) return 'root';
      window.__atlasBack();
      return 'handled';
    })()
"""

/**
 * Takes the window edge to edge, so the star field runs behind the system bars.
 *
 * `application.android_main_activity` in Dioxus.toml points dx at this file, which it copies
 * verbatim. Nothing here is substituted, so the `BuildConfig` alias must match
 * `bundle.identifier`.
 */
class MainActivity : WryActivity() {
    private var webView: WebView? = null

    /**
     * WryActivity handles back from `onKeyDown`, which never fires at SDK 35 and above:
     * predictive back is on by default there and stops `KEYCODE_BACK` and `onBackPressed()`
     * being dispatched. [backCallback] owns back instead.
     */
    override val handleBackNavigation: Boolean = false

    /** Steps the app back, finishing the activity only once it has nowhere left to go. */
    private val backCallback = object : OnBackPressedCallback(true) {
        override fun handleOnBackPressed() {
            val view = webView
            if (view == null) {
                leave()
                return
            }
            view.evaluateJavascript(HANDLE_BACK) { answer ->
                if (answer != "\"handled\"") leave()
            }
        }
    }

    private fun leave() {
        backCallback.isEnabled = false
        onBackPressedDispatcher.onBackPressed()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        onBackPressedDispatcher.addCallback(this, backCallback)

        window.setBackgroundDrawable(ColorDrawable(SKY))

        WindowCompat.setDecorFitsSystemWindows(window, false)

        // Applies on API 31–34. From Android 15 (API 35) the system enforces transparent
        // bars and ignores both.
        @Suppress("DEPRECATION")
        run {
            window.statusBarColor = Color.TRANSPARENT
            window.navigationBarColor = Color.TRANSPARENT
        }

        // Suppresses the translucent scrim the platform otherwise paints behind the bars.
        window.isStatusBarContrastEnforced = false
        window.isNavigationBarContrastEnforced = false

        // Light icons and gesture pill, over a dark sky.
        WindowCompat.getInsetsController(window, window.decorView).apply {
            isAppearanceLightStatusBars = false
            isAppearanceLightNavigationBars = false
        }

        // The web layout roots on a fixed, full-window box, which the keyboard would cover
        // rather than shrink. Insetting the content view shrinks the layout viewport with it.
        val content = findViewById<View>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
            view.setPadding(0, 0, 0, insets.getInsets(WindowInsetsCompat.Type.ime()).bottom)
            insets
        }
    }

    /** WebView follows neither the system font scale nor the window's background on its own. */
    override fun onWebViewCreate(webView: WebView) {
        this.webView = webView
        webView.setBackgroundColor(SKY)
        webView.settings.textZoom = (resources.configuration.fontScale * 100).toInt()
        // The dose log lives in local storage, which WebView refuses by default: reading it
        // throws rather than returning nothing, and the app reports itself unable to save.
        webView.settings.domStorageEnabled = true
    }
}
