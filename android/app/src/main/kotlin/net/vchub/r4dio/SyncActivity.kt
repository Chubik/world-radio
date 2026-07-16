package net.vchub.r4dio

import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.view.View
import android.widget.EditText
import android.widget.ImageView
import android.widget.TextView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import com.google.zxing.BarcodeFormat
import com.google.zxing.EncodeHintType
import com.google.zxing.MultiFormatWriter
import com.google.zxing.qrcode.decoder.ErrorCorrectionLevel
import com.journeyapps.barcodescanner.BarcodeEncoder
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class SyncActivity : ComponentActivity() {
    private val favStore by lazy { FavStore(this) }
    private val syncClient = SyncClient()
    private var creating = false

    private val scanner = registerForActivityResult(ScanContract()) { result ->
        val contents = result.contents
        when {
            contents == null -> toast("scan cancelled")
            !contents.startsWith("r4-") -> toast("not an r4dio key")
            else -> lifecycleScope.launch {
                favStore.setSyncKey(contents)
                render()
                toast("key imported")
            }
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_sync)
        applyStatusBarInset()
        wire()
        render()
    }

    // pad the root below the status bar so the app-bar (and its DONE button) isn't
    // drawn under the clock, which made DONE untappable.
    private fun applyStatusBarInset() {
        val root = findViewById<View>(android.R.id.content)
        androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(root) { v, insets ->
            val top = insets.getInsets(androidx.core.view.WindowInsetsCompat.Type.statusBars()).top
            v.setPadding(v.paddingLeft, top, v.paddingRight, v.paddingBottom)
            insets
        }
    }

    private fun wire() {
        findViewById<View>(R.id.done).setOnClickListener { finish() }
        findViewById<View>(R.id.use_key).setOnClickListener {
            val k = findViewById<EditText>(R.id.key_input).text.toString().trim()
            when (k.startsWith("r4-")) {
                false -> toast("invalid key")
                true -> lifecycleScope.launch { favStore.setSyncKey(k); render(); toast("key set") }
            }
        }
        findViewById<View>(R.id.create).setOnClickListener { view ->
            // guard against double-taps: each POST /account mints a NEW account, so
            // a second tap while the first is in flight would create an orphan.
            if (creating) {
                return@setOnClickListener
            }
            creating = true
            view.isEnabled = false
            lifecycleScope.launch {
                val k = withContext(Dispatchers.IO) { syncClient.createAccount() }
                when (k) {
                    null -> toast("could not create account")
                    else -> { favStore.setSyncKey(k); render(); toast("account created") }
                }
                creating = false
                view.isEnabled = true
            }
        }
        findViewById<View>(R.id.scan).setOnClickListener {
            scanner.launch(
                ScanOptions()
                    .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setOrientationLocked(false)
                    .setCaptureActivity(PortraitCaptureActivity::class.java)
                    .setBeepEnabled(false)
                    .setPrompt("point at the r4dio qr · back to cancel"),
            )
        }
        findViewById<View>(R.id.copy).setOnClickListener {
            lifecycleScope.launch {
                val k = favStore.syncKey() ?: return@launch
                val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("r4dio sync key", k))
                toast("copied")
            }
        }
        findViewById<View>(R.id.logout).setOnClickListener {
            lifecycleScope.launch { favStore.setSyncKey(null); render(); toast("logged out") }
        }
        findViewById<View>(R.id.delete).setOnClickListener {
            AlertDialog.Builder(this@SyncActivity)
                .setTitle("delete account?")
                .setMessage("this permanently deletes your sync account on the server. your local favourites stay on this device.")
                .setPositiveButton("delete") { _, _ ->
                    lifecycleScope.launch {
                        val k = favStore.syncKey() ?: return@launch
                        withContext(Dispatchers.IO) { syncClient.delete(k) }
                        favStore.setSyncKey(null); render(); toast("account deleted")
                    }
                }
                .setNegativeButton("cancel", null)
                .show()
        }
        findViewById<View>(R.id.excluded_countries).setOnClickListener {
            lifecycleScope.launch {
                val all = countryChoices()
                val current = favStore.currentExcluded()
                val checked = BooleanArray(all.size) { all[it] in current }
                AlertDialog.Builder(this@SyncActivity)
                    .setTitle("hide countries")
                    .setMultiChoiceItems(all.toTypedArray(), checked) { _, which, isChecked ->
                        checked[which] = isChecked
                    }
                    .setPositiveButton("save") { _, _ ->
                        lifecycleScope.launch {
                            val sel = all.filterIndexed { i, _ -> checked[i] }.toSet()
                            favStore.setExcluded(sel)
                            triggerSync()
                            toast("saved")
                        }
                    }
                    .setNegativeButton("cancel", null)
                    .show()
            }
        }
    }

    private fun countryChoices(): List<String> = countryCodes.sorted()

    private fun triggerSync() {
        startService(
            android.content.Intent(this, PlaybackService::class.java)
                .setAction(ACTION_SYNC_NOW)
        )
    }

    private val countryCodes = listOf(
        "AR", "AT", "AU", "BE", "BR", "CA", "CH", "CL", "CN", "CO",
        "CZ", "DE", "DK", "EG", "ES", "FI", "FR", "GB", "GR", "HU",
        "ID", "IE", "IL", "IN", "IT", "JP", "KR", "MX", "NL", "NO",
        "NZ", "PL", "PT", "RO", "SE", "TH", "TR", "UA", "US", "ZA",
    )

    private fun render() {
        lifecycleScope.launch {
            val key = favStore.syncKey()
            val hasKey = key != null
            // swap whole states rather than toggling each control individually.
            findViewById<View>(R.id.state_a).visibility = vis(!hasKey)
            findViewById<View>(R.id.state_b).visibility = vis(hasKey)
            if (!hasKey) {
                return@launch
            }
            findViewById<TextView>(R.id.key_shown).text = key
            // encode with a quiet-zone margin and high error correction — without a
            // margin zxing's own scanner fails to lock onto the finder patterns even
            // though phone cameras cope.
            val hints = mapOf(
                EncodeHintType.MARGIN to 2,
                EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.H,
            )
            val matrix = MultiFormatWriter().encode(key, BarcodeFormat.QR_CODE, 500, 500, hints)
            val bmp = BarcodeEncoder().createBitmap(matrix)
            findViewById<ImageView>(R.id.qr).setImageBitmap(bmp)
        }
    }

    private fun vis(show: Boolean) = when (show) {
        true -> android.view.View.VISIBLE
        false -> android.view.View.GONE
    }

    private fun toast(msg: String) {
        Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
    }
}
