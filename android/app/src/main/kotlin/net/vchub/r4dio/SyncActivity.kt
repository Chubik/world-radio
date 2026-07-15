package net.vchub.r4dio

import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.os.Bundle
import android.widget.Button
import android.widget.EditText
import android.widget.ImageView
import android.widget.TextView
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import com.google.zxing.BarcodeFormat
import com.journeyapps.barcodescanner.BarcodeEncoder
import com.journeyapps.barcodescanner.ScanContract
import com.journeyapps.barcodescanner.ScanOptions
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

class SyncActivity : ComponentActivity() {
    private val favStore by lazy { FavStore(this) }
    private val syncClient = SyncClient()

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
        wire()
        render()
    }

    private fun wire() {
        findViewById<Button>(R.id.use_key).setOnClickListener {
            val k = findViewById<EditText>(R.id.key_input).text.toString().trim()
            when (k.startsWith("r4-")) {
                false -> toast("invalid key")
                true -> lifecycleScope.launch { favStore.setSyncKey(k); render(); toast("key set") }
            }
        }
        findViewById<Button>(R.id.create).setOnClickListener {
            lifecycleScope.launch {
                val k = withContext(Dispatchers.IO) { syncClient.createAccount() }
                when (k) {
                    null -> toast("could not create account")
                    else -> { favStore.setSyncKey(k); render(); toast("account created") }
                }
            }
        }
        findViewById<Button>(R.id.scan).setOnClickListener {
            scanner.launch(
                ScanOptions()
                    .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                    .setOrientationLocked(false)
                    .setCaptureActivity(PortraitCaptureActivity::class.java)
                    .setBeepEnabled(false)
                    .setPrompt("point at the r4dio qr · back to cancel"),
            )
        }
        findViewById<Button>(R.id.copy).setOnClickListener {
            lifecycleScope.launch {
                val k = favStore.syncKey() ?: return@launch
                val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
                cm.setPrimaryClip(ClipData.newPlainText("r4dio sync key", k))
                toast("copied")
            }
        }
        findViewById<Button>(R.id.logout).setOnClickListener {
            lifecycleScope.launch { favStore.setSyncKey(null); render(); toast("logged out") }
        }
        findViewById<Button>(R.id.delete).setOnClickListener {
            lifecycleScope.launch {
                val k = favStore.syncKey()
                when (k) {
                    null -> {}
                    else -> {
                        withContext(Dispatchers.IO) { syncClient.delete(k) }
                        favStore.setSyncKey(null); render(); toast("account deleted")
                    }
                }
            }
        }
        findViewById<Button>(R.id.excluded_countries).setOnClickListener {
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
            findViewById<TextView>(R.id.key_shown).apply {
                text = key ?: ""
                visibility = vis(hasKey)
            }
            findViewById<Button>(R.id.copy).visibility = vis(hasKey)
            findViewById<Button>(R.id.logout).visibility = vis(hasKey)
            findViewById<Button>(R.id.delete).visibility = vis(hasKey)
            val qr = findViewById<ImageView>(R.id.qr)
            when (hasKey) {
                false -> qr.visibility = android.view.View.GONE
                true -> {
                    val bmp = BarcodeEncoder().encodeBitmap(key, BarcodeFormat.QR_CODE, 500, 500)
                    qr.setImageBitmap(bmp)
                    qr.visibility = android.view.View.VISIBLE
                }
            }
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
