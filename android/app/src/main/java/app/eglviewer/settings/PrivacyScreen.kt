package app.eglviewer.settings

import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.res.stringResource
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.compose.ui.unit.sp
import app.eglviewer.R
import app.eglviewer.theme.EglTokens

/**
 * In-app privacy notes (A8 ≈ D-100): no tracking, libraries stay on the phone, biometrics
 * and export passphrases never leave the device.
 */
@Composable
fun PrivacyScreen(onBack: () -> Unit) {
    Column(
        modifier = Modifier
            .fillMaxSize()
            .background(EglTokens.Background),
    ) {
        SettingsTopBar(title = stringResource(R.string.privacy_title), onBack = onBack)
        Column(
            modifier = Modifier
                .fillMaxSize()
                .verticalScroll(rememberScrollState())
                .padding(horizontal = 20.dp)
                .padding(bottom = 28.dp),
            verticalArrangement = Arrangement.spacedBy(16.dp),
        ) {
            PrivacyParagraph(stringResource(R.string.privacy_intro))
            PrivacyHeading(stringResource(R.string.privacy_data_heading))
            PrivacyParagraph(stringResource(R.string.privacy_data_body))
            PrivacyHeading(stringResource(R.string.privacy_device_heading))
            PrivacyParagraph(stringResource(R.string.privacy_device_body))
            PrivacyHeading(stringResource(R.string.privacy_network_heading))
            PrivacyParagraph(stringResource(R.string.privacy_network_body))
        }
    }
}

@Composable
private fun PrivacyHeading(text: String) {
    Text(
        text = text,
        color = EglTokens.Foreground,
        fontSize = 15.sp,
        fontWeight = FontWeight.SemiBold,
    )
}

@Composable
private fun PrivacyParagraph(text: String) {
    Text(
        text = text,
        color = EglTokens.MutedForeground,
        fontSize = 14.sp,
        lineHeight = 20.sp,
    )
}
