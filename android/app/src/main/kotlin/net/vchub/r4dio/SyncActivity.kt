package net.vchub.r4dio

import android.app.AlertDialog
import android.content.ClipData
import android.content.ClipboardManager
import android.content.Context
import android.graphics.Bitmap
import android.os.Bundle
import android.widget.Toast
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
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
import net.vchub.r4dio.ui.DEFAULT_THEME
import net.vchub.r4dio.ui.R4dioTheme
import net.vchub.r4dio.ui.SyncScreen
import net.vchub.r4dio.ui.resolveTheme

/**
 * account and backup. still its own activity rather than a tab, because three of
 * its actions are system contracts (camera, create-document, open-document) that
 * must be registered against an activity — but the screen itself is compose now,
 * so this is the last xml layout gone.
 */
class SyncActivity : ComponentActivity() {
    private val favStore by lazy { FavStore(this) }
    private val syncClient = SyncClient()

    private var key by mutableStateOf<String?>(null)
    private var qr by mutableStateOf<Bitmap?>(null)
    private var hidden by mutableStateOf<Set<String>>(emptySet())

    // each create mints a NEW account server-side, so a second tap while the
    // first is in flight would leave an orphan behind.
    private var creating by mutableStateOf(false)

    private val scanner = registerForActivityResult(ScanContract()) { result ->
        when (val outcome = scanOutcome(result.contents)) {
            ScanOutcome.Cancelled -> toast("scan cancelled")
            ScanOutcome.NotAKey -> toast("not an r4dio key")
            is ScanOutcome.Linked -> lifecycleScope.launch {
                linkAndMerge(outcome.key)
                render()
                toast("key imported")
            }
        }
    }

    private val exportPicker = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.CreateDocument("application/json"),
    ) { uri ->
        when (uri) {
            null -> {}
            else -> lifecycleScope.launch {
                val body = BackupFile.encode(
                    key = favStore.syncKey(),
                    favs = favStore.currentFavUuids(),
                    cached = favStore.currentCachedFavs(),
                    blocked = favStore.currentBlocked(),
                    excluded = favStore.currentExcluded(),
                )
                val ok = withContext(Dispatchers.IO) {
                    runCatching {
                        contentResolver.openOutputStream(uri)?.use { it.write(body.toByteArray()) }
                    }.isSuccess
                }
                toast(getString(if (ok) R.string.sync_exported else R.string.sync_export_failed))
            }
        }
    }

    private val importPicker = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.OpenDocument(),
    ) { uri ->
        when (uri) {
            null -> {}
            else -> lifecycleScope.launch {
                val text = withContext(Dispatchers.IO) {
                    runCatching {
                        contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() }
                    }.getOrNull()
                }
                val backup = text?.let { BackupFile.decode(it) }
                when (backup) {
                    null -> toast(getString(R.string.sync_import_failed))
                    else -> {
                        favStore.restore(backup)
                        render()
                        toast(getString(R.string.sync_imported, backup.favs.size))
                        triggerSync()
                    }
                }
            }
        }
    }

    private suspend fun linkAndMerge(key: String) {
        favStore.setSyncKey(key)
        val profile = favStore.profile()
        val plays = HistoryQueue.records(favStore.pendingPlays())
        val local = profile.outgoing(
            favs = favStore.currentFavUuids().toList(),
            blocked = favStore.currentBlocked().toList(),
            excluded = favStore.currentExcluded().toList(),
            plays = plays,
        )
        val server = withContext(Dispatchers.IO) { syncClient.pull(key) } ?: SyncData(emptyList(), emptyList())
        // the id lists union; the profile does not — it is last-write-wins, so the
        // newer side of each field wins and the merged profile is what gets pushed.
        val merged = SyncMerge.mergedData(local, server)
        favStore.applyMerged(merged.favs.toSet(), merged.blocked.toSet(), merged.excluded_countries.toSet())
        val nextProfile = profile.applyRemote(server)
        when (nextProfile == profile) {
            true -> {}
            false -> favStore.applyProfile(nextProfile)
        }
        val push = nextProfile.outgoing(
            favs = merged.favs,
            blocked = merged.blocked,
            excluded = merged.excluded_countries,
            plays = plays,
        )
        val pushed = withContext(Dispatchers.IO) { syncClient.push(key, push) }
        // only drop what the server actually took: a failed push must leave the
        // plays queued for the next sync rather than losing them.
        when (pushed) {
            null -> {}
            else -> favStore.drainPlays(plays)
        }
        // linking a device can pull in a different excluded-country set than this
        // device had; applyMerged() already reset the sync stamp if so, but only the
        // running service's syncNow()/refreshIfStale() acts on that reset.
        triggerSync()
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent {
            val synced by favStore.theme.collectAsState(initial = "")
            R4dioTheme(resolveTheme(synced, DEFAULT_THEME)) {
                SyncScreen(
                    key = key,
                    qr = qr,
                    hiddenCountries = hidden,
                    busy = creating,
                    onUseKey = { typed ->
                        when (isSyncKey(typed)) {
                            false -> toast("invalid key")
                            true -> lifecycleScope.launch {
                                linkAndMerge(typed)
                                render()
                                toast("key set")
                            }
                        }
                    },
                    onCreate = ::createAccount,
                    onScan = ::scan,
                    onCopy = ::copyKey,
                    onLogOut = {
                        lifecycleScope.launch {
                            favStore.setSyncKey(null)
                            render()
                            toast("logged out")
                        }
                    },
                    onDelete = ::confirmDelete,
                    onExport = { exportPicker.launch(getString(R.string.sync_export_name)) },
                    // some file providers hand json back as octet-stream, so accept both
                    onImport = {
                        importPicker.launch(
                            arrayOf("application/json", "application/octet-stream", "text/plain"),
                        )
                    },
                    onHiddenCountries = { next ->
                        lifecycleScope.launch {
                            favStore.setExcluded(next)
                            hidden = favStore.currentExcluded()
                            triggerSync()
                        }
                    },
                    onFeedback = ::openFeedback,
                    onClose = { finish() },
                )
            }
        }
        render()
    }

    private fun createAccount() {
        if (creating) {
            return
        }
        creating = true
        lifecycleScope.launch {
            val k = withContext(Dispatchers.IO) { syncClient.createAccount() }
            when (k) {
                null -> toast("could not create account")
                else -> {
                    favStore.setSyncKey(k)
                    render()
                    toast("account created")
                }
            }
            creating = false
        }
    }

    private fun scan() {
        scanner.launch(
            ScanOptions()
                .setDesiredBarcodeFormats(ScanOptions.QR_CODE)
                .setOrientationLocked(false)
                .setCaptureActivity(PortraitCaptureActivity::class.java)
                .setBeepEnabled(false)
                .setPrompt("point at the r4dio qr · back to cancel"),
        )
    }

    private fun copyKey() {
        lifecycleScope.launch {
            val k = favStore.syncKey() ?: return@launch
            val cm = getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
            cm.setPrimaryClip(ClipData.newPlainText("r4dio sync key", k))
            toast("copied")
        }
    }

    private fun confirmDelete() {
        AlertDialog.Builder(this)
            .setTitle("delete account?")
            .setMessage("this permanently deletes your sync account on the server. your local favourites stay on this device.")
            .setPositiveButton("delete") { _, _ ->
                lifecycleScope.launch {
                    val k = favStore.syncKey() ?: return@launch
                    withContext(Dispatchers.IO) { syncClient.delete(k) }
                    favStore.setSyncKey(null)
                    render()
                    toast("account deleted")
                }
            }
            .setNegativeButton("cancel", null)
            .show()
    }

    private fun openFeedback() {
        val version = runCatching {
            packageManager.getPackageInfo(packageName, 0).versionName
        }.getOrNull().orEmpty()
        val intent = android.content.Intent(android.content.Intent.ACTION_SENDTO).apply {
            data = android.net.Uri.parse("mailto:support@r4dio.net")
            putExtra(android.content.Intent.EXTRA_SUBJECT, "r4dio feedback (v$version)")
        }
        runCatching { startActivity(intent) }
            .onFailure { toast("email support@r4dio.net") }
    }

    private fun triggerSync() {
        startService(
            android.content.Intent(this, PlaybackService::class.java)
                .setAction(ACTION_SYNC_NOW),
        )
    }

    private fun render() {
        lifecycleScope.launch {
            val k = favStore.syncKey()
            key = k
            hidden = favStore.currentExcluded()
            qr = when (k) {
                null -> null
                else -> withContext(Dispatchers.Default) { qrFor(k) }
            }
        }
    }

    /**
     * encode with a quiet-zone margin and high error correction — without a
     * margin zxing's own scanner fails to lock onto the finder patterns even
     * though phone cameras cope. this is the code another device points at, so
     * it has to be readable by our own scanner, not merely by a good one.
     */
    private fun qrFor(key: String): Bitmap? = runCatching {
        val hints = mapOf(
            EncodeHintType.MARGIN to 2,
            EncodeHintType.ERROR_CORRECTION to ErrorCorrectionLevel.H,
        )
        val matrix = MultiFormatWriter().encode(key, BarcodeFormat.QR_CODE, 500, 500, hints)
        BarcodeEncoder().createBitmap(matrix)
    }.getOrNull()

    private fun toast(msg: String) {
        Toast.makeText(this, msg, Toast.LENGTH_SHORT).show()
    }
}
